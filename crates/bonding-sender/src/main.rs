//! `bonding-sender SRT_LISTEN_PORT SRTLA_HOST SRTLA_PORT BIND_IPS_FILE`: a drop-in for upstream
//! `srtla_send`.

use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr, UdpSocket};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use bonding_protocol::srtla::{GroupId, ID_LEN};
use bonding_sender::server::{SenderConfig, UdpIo, WAKE_INTERVAL, read_source_list, serve};
use bonding_sender::srtla::Sender;

const RANDOMNESS_PATH: &str = "/dev/urandom";

fn random_group_id() -> std::io::Result<GroupId> {
    let mut id = [0u8; ID_LEN];
    File::open(RANDOMNESS_PATH)?.read_exact(&mut id)?;
    Ok(id)
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.as_slice() == ["-v"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let config = SenderConfig::from_arguments(arguments).map_err(|error| error.to_string())?;
    let sources = read_source_list(&config.source_list)
        .map_err(|error| format!("cannot read {}: {error}", config.source_list.display()))?;
    let group_id = random_group_id().map_err(|error| format!("no randomness: {error}"))?;
    let listen = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.listen_port))
        .map_err(|error| format!("cannot listen on port {}: {error}", config.listen_port))?;
    let mut io = UdpIo::new(listen);
    let mut sender = Sender::new(group_id, &config.receivers)
        .ok_or_else(|| "no address for the receiver".to_owned())?;
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
    if let Some(receiver) = sender.receiver() {
        eprintln!(
            "bonding-sender: SRT on port {}, receiver at {receiver}",
            config.listen_port
        );
    }
    serve(
        &mut sender,
        &mut io,
        &config.source_list,
        WAKE_INTERVAL,
        net_sys::hangup_flag(),
        &AtomicBool::new(false),
    )
    .map_err(|error| format!("bonding-sender stopped: {error}"))
}
