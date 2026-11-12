//! `bonding-receiver LISTEN_PORT SRT_HOST SRT_PORT`: a drop-in for upstream `srtla_rec`.

use std::net::{Ipv4Addr, UdpSocket};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use bonding_receiver::server::{ServerConfig, UdpIo, WAKE_INTERVAL, serve};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.as_slice() == ["-v"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let config = match ServerConfig::from_arguments(&arguments) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let listen = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.listen_port)) {
        Ok(socket) => socket,
        Err(error) => {
            eprintln!("cannot listen on port {}: {error}", config.listen_port);
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "bonding-receiver: listening on {}, forwarding to SRT at {}",
        config.listen_port, config.srt_address
    );
    let mut io = UdpIo::new(listen, config.srt_address);
    match serve(&mut io, WAKE_INTERVAL, &AtomicBool::new(false)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bonding-receiver stopped: {error}");
            ExitCode::FAILURE
        }
    }
}
