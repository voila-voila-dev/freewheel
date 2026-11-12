//! The sender on real UDP sockets, against our own receiver, everything on localhost: SRT caller
//! → bonding-sender → bonding-receiver → SRT listener, and back.

use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use bonding_protocol::srtla::{ID_LEN, PacketType};
use bonding_sender::server::{SenderConfig, UdpIo, serve};
use bonding_sender::srtla::Sender;

type TestResult = Result<(), Box<dyn Error>>;

const READ_TIMEOUT: Duration = Duration::from_secs(5);

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

fn data(sequence_number: u32) -> Vec<u8> {
    let mut packet = sequence_number.to_be_bytes().to_vec();
    packet.extend([0xAB; 1_312]);
    packet
}

#[test]
fn a_stream_crosses_our_sender_and_our_receiver() -> TestResult {
    let stop = Arc::new(AtomicBool::new(false));

    let srt_listener = local_socket()?;
    let receiver_socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    let receiver_address = v4(receiver_socket.local_addr()?)?;
    let mut receiver_io =
        bonding_receiver::server::UdpIo::new(receiver_socket, v4(srt_listener.local_addr()?)?);
    let receiver = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            bonding_receiver::server::serve(&mut receiver_io, Duration::from_millis(20), &stop)
        })
    };

    let sender_socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    let sender_address = sender_socket.local_addr()?;
    let mut sender_io = UdpIo::new(sender_socket);
    let mut sender = Sender::new([0x42; ID_LEN], &[receiver_address]).ok_or("no receiver")?;
    sender.reload_sources(&[Ipv4Addr::LOCALHOST], &mut sender_io);
    assert_eq!(sender.open_links(&mut sender_io), 1);
    let sender_thread = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            let reload = AtomicBool::new(false);
            serve(
                &mut sender,
                &mut sender_io,
                std::path::Path::new("/nonexistent"),
                Duration::from_millis(20),
                &reload,
                &stop,
            )
        })
    };

    let caller = local_socket()?;
    let mut first = None;
    for attempt in 0..100 {
        caller.send_to(&data(attempt), sender_address)?;
        srt_listener.set_read_timeout(Some(Duration::from_millis(50)))?;
        if let Ok((bytes, from)) = receive(&srt_listener) {
            first = Some((bytes, from));
            break;
        }
    }
    let (bytes, group_socket) = first.ok_or("nothing crossed after registration")?;
    assert_eq!(bytes.len(), 1_316, "forwarded byte for byte");

    srt_listener.set_read_timeout(Some(READ_TIMEOUT))?;
    caller.send_to(&data(1_000), sender_address)?;
    assert_eq!(receive(&srt_listener)?.0, data(1_000));

    let mut handshake = PacketType::SrtHandshake.raw().to_be_bytes().to_vec();
    handshake.extend([0; 62]);
    srt_listener.send_to(&handshake, group_socket)?;
    assert_eq!(
        receive(&caller)?.0,
        handshake,
        "the listener's answer reaches the caller"
    );

    stop.store(true, Ordering::Relaxed);
    sender_thread
        .join()
        .map_err(|_| "sender thread panicked")??;
    receiver.join().map_err(|_| "receiver thread panicked")??;
    Ok(())
}

#[test]
fn arguments_are_upstreams() -> TestResult {
    let arguments = ["6001", "127.0.0.1", "5000", "/tmp/srtla_ips"].map(String::from);
    let config = SenderConfig::from_arguments(&arguments)?;
    assert_eq!(config.listen_port, 6_001);
    assert_eq!(
        config.receivers,
        vec![SocketAddrV4::new(Ipv4Addr::LOCALHOST, 5_000)]
    );
    assert!(SenderConfig::from_arguments(&arguments[..3]).is_err());
    assert!(
        SenderConfig::from_arguments(&["0", "127.0.0.1", "5000", "x"].map(String::from)).is_err()
    );
    Ok(())
}
