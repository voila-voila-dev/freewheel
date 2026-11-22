//! `freewheel-sender SRT_LISTEN_PORT RECEIVER_HOST RECEIVER_PORT BIND_IPS_FILE`: our own
//! transport (M1b), with the same command line as `bonding-sender`.
//!
//! With `FREEWHEEL_CAPACITY_SOCKET=<path>` in its environment, it also sends a capacity report
//! (`bitrate_control::CapacityReport`, JSON) to that Unix datagram socket every 100 ms, for
//! the encoder's bitrate.

use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use bonding_sender::freewheel::Sender;
use bonding_sender::server::{
    CapacityOutput, FREEWHEEL_WAKE_INTERVAL, SenderConfig, UdpIo, random_session_id,
    read_source_list, serve_freewheel,
};

const CAPACITY_SOCKET_VARIABLE: &str = "FREEWHEEL_CAPACITY_SOCKET";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let config = SenderConfig::from_arguments(arguments).map_err(|error| {
        error
            .to_string()
            .replace("bonding-sender", "freewheel-sender")
    })?;
    let sources = read_source_list(&config.source_list)
        .map_err(|error| format!("cannot read {}: {error}", config.source_list.display()))?;
    let session_id = random_session_id().map_err(|error| format!("no randomness: {error}"))?;
    let listen = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.listen_port))
        .map_err(|error| format!("cannot listen on port {}: {error}", config.listen_port))?;
    let receiver = *config
        .receivers
        .first()
        .ok_or_else(|| "no address for the receiver".to_owned())?;
    let mut io = UdpIo::new(listen);
    let mut sender = Sender::new(session_id, receiver);
    if sender.reload_sources(&sources, &mut io) == 0 {
        return Err(format!(
            "no IPv4 address in {}",
            config.source_list.display()
        ));
    }
    if sender.open_links(&mut io) == 0 {
        return Err(format!(
            "cannot bind to any address in {}",
            config.source_list.display()
        ));
    }
    net_sys::catch_hangup().map_err(|error| format!("cannot catch SIGHUP: {error}"))?;
    let capacity_path = std::env::var_os(CAPACITY_SOCKET_VARIABLE).map(PathBuf::from);
    let capacity = capacity_path
        .as_deref()
        .map(CapacityOutput::new)
        .transpose()
        .map_err(|error| format!("cannot open a Unix socket: {error}"))?;
    eprintln!(
        "freewheel-sender: SRT on port {}, receiver at {receiver}{}",
        config.listen_port,
        capacity_path
            .as_deref()
            .map(|path| format!(", capacity to {}", path.display()))
            .unwrap_or_default()
    );
    serve_freewheel(
        &mut sender,
        &mut io,
        &config.source_list,
        FREEWHEEL_WAKE_INTERVAL,
        net_sys::hangup_flag(),
        &AtomicBool::new(false),
        capacity.as_ref(),
    )
    .map_err(|error| format!("freewheel-sender stopped: {error}"))
}
