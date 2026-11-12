//! The server on real UDP sockets: two links, a fake SRT listener, everything on localhost.

mod support;

use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use bonding_protocol::srtla::PacketType;
use bonding_receiver::server::{ServerConfig, UdpIo, serve};
use support::{TestResult, answer, data, id_in, reg1, reg2, srt_control};

const READ_TIMEOUT: Duration = Duration::from_secs(3);

fn local_socket() -> Result<UdpSocket, Box<dyn Error>> {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    socket.set_read_timeout(Some(READ_TIMEOUT))?;
    Ok(socket)
}

fn v4(address: SocketAddr) -> Result<SocketAddrV4, Box<dyn Error>> {
    match address {
        SocketAddr::V4(v4) => Ok(v4),
        SocketAddr::V6(_) => Err("not IPv4".into()),
    }
}

fn receive(socket: &UdpSocket) -> Result<(Vec<u8>, SocketAddr), Box<dyn Error>> {
    let mut buffer = [0u8; 1_500];
    let (len, from) = socket.recv_from(&mut buffer)?;
    Ok((buffer[..len].to_vec(), from))
}

#[test]
fn registers_forwards_and_answers_on_real_sockets() -> TestResult {
    let listen = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    let receiver_address = listen.local_addr()?;
    let srt_listener = local_socket()?;
    let mut io = UdpIo::new(listen, v4(srt_listener.local_addr()?)?);
    let stop = Arc::new(AtomicBool::new(false));
    let server = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || serve(&mut io, Duration::from_millis(20), &stop))
    };

    let links = [local_socket()?, local_socket()?];
    links[0].send_to(&reg1(0x11), receiver_address)?;
    let id = id_in(&receive(&links[0])?.0).ok_or("no REG2")?;
    for link in &links {
        link.send_to(&reg2(&id), receiver_address)?;
        assert_eq!(receive(link)?.0, answer(PacketType::Reg3));
    }

    links[1].send_to(&data(42), receiver_address)?;
    let (forwarded, group_socket) = receive(&srt_listener)?;
    assert_eq!(forwarded, data(42));

    let ack = srt_control(PacketType::SrtAck);
    srt_listener.send_to(&ack, group_socket)?;
    for link in &links {
        assert_eq!(receive(link)?.0, ack, "SRT ACKs go on every link");
    }
    let nak = srt_control(PacketType::SrtNak);
    srt_listener.send_to(&nak, group_socket)?;
    assert_eq!(
        receive(&links[1])?.0,
        nak,
        "on the link that last carried data"
    );

    let mut keepalive = answer(PacketType::Keepalive);
    keepalive.extend([0xEE; 1_600]);
    links[0].send_to(&keepalive, receiver_address)?;
    assert_eq!(
        receive(&links[0])?.0.len(),
        1_500,
        "truncated to 1 500 bytes, like upstream"
    );

    stop.store(true, Ordering::Relaxed);
    server.join().map_err(|_| "server thread panicked")??;
    Ok(())
}

#[test]
fn arguments_are_upstreams() -> TestResult {
    let arguments = ["5000", "127.0.0.1", "5002"].map(String::from);
    let config = ServerConfig::from_arguments(&arguments)?;
    assert_eq!(config.listen_port, 5_000);
    assert_eq!(
        config.srt_address,
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5_002)
    );
    assert!(ServerConfig::from_arguments(&arguments[..2]).is_err());
    assert!(ServerConfig::from_arguments(&["0", "127.0.0.1", "5002"].map(String::from)).is_err());
    assert!(
        ServerConfig::from_arguments(&["5000", "127.0.0.1", "70000"].map(String::from)).is_err()
    );
    Ok(())
}
