//! `freewheel-receiver LISTEN_PORT SRT_HOST SRT_PORT`: our own transport (M1b), with the
//! same command line as `bonding-receiver`.
//!
//! With `FREEWHEEL_RECORDING_DIR=<directory>`, it writes the recording each session uploads
//! (the background class) there; its other numbers are `FREEWHEEL_RECORDING_*` variables
//! (`bonding_receiver::freewheel::RecordingSettings`). Without it, background data is counted,
//! acknowledged and dropped.

use std::net::{Ipv4Addr, UdpSocket};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use bonding_receiver::freewheel::RecordingSettings;
use bonding_receiver::server::{FREEWHEEL_WAKE_INTERVAL, ServerConfig, UdpIo, serve_freewheel};

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let config = match ServerConfig::from_arguments(&arguments) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "{}",
                error
                    .to_string()
                    .replace("bonding-receiver", "freewheel-receiver")
            );
            return ExitCode::from(2);
        }
    };
    let recording = match RecordingSettings::from_environment(|name| std::env::var(name).ok()) {
        Ok(recording) => recording,
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
        "freewheel-receiver: listening on {}, forwarding to SRT at {}{}",
        config.listen_port,
        config.srt_address,
        recording
            .as_ref()
            .map(|settings| format!(", recording {settings:?}"))
            .unwrap_or_default()
    );
    let mut io = UdpIo::new(listen, config.srt_address);
    match serve_freewheel(&mut io, FREEWHEEL_WAKE_INTERVAL, &AtomicBool::new(false)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("freewheel-receiver stopped: {error}");
            ExitCode::FAILURE
        }
    }
}
