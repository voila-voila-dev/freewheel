//! `freewheel-receiver LISTEN_PORT SRT_HOST SRT_PORT`: our own transport (M1b), with the
//! same command line as `bonding-receiver`.
//!
//! With `FREEWHEEL_RECORDING_DIR=<directory>`, it writes the recording each session uploads
//! (the background class) there; its other numbers are `FREEWHEEL_RECORDING_*` variables
//! (`bonding_receiver::freewheel::RecordingSettings`). Without it, background data is counted,
//! acknowledged and dropped.
//!
//! Encrypted by default (wire format v1): `FREEWHEEL_PRIVATE_KEY` (the server's key file),
//! `FREEWHEEL_AUTHORIZED_KEYS` (the boxes' public keys, one per line), `FREEWHEEL_CIPHER`,
//! `FREEWHEEL_REPLAY_WINDOW_PACKETS` (`bonding_receiver::freewheel::EncryptionSettings`).
//! `FREEWHEEL_PLAINTEXT=1`: v0 in clear, no key.

use std::net::{Ipv4Addr, UdpSocket};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use bonding_receiver::freewheel::{EncryptionSettings, RecordingSettings};
use bonding_receiver::server::{
    FREEWHEEL_WAKE_INTERVAL, RecordingWriter, ServerConfig, UdpIo, serve_freewheel,
};

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
    let encryption = match EncryptionSettings::from_environment(|name| std::env::var(name).ok()) {
        Ok(encryption) => encryption,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let keys = match encryption
        .as_ref()
        .map(EncryptionSettings::read_keys)
        .transpose()
    {
        Ok(keys) => keys,
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
        "freewheel-receiver: listening on {}, forwarding to SRT at {}, {}{}",
        config.listen_port,
        config.srt_address,
        keys.as_ref().map_or_else(
            || "in clear".to_owned(),
            |keys| format!(
                "encrypted ({}, {} authorised boxes, replay window {} packets)",
                keys.cipher.name(),
                keys.authorized.len(),
                keys.replay_window_packets
            )
        ),
        recording
            .as_ref()
            .map(|settings| format!(", recording {settings:?}"))
            .unwrap_or_default()
    );
    let mut io = UdpIo::new(listen, config.srt_address);
    if let Some(settings) = &recording {
        match RecordingWriter::start(settings) {
            Ok(writer) => io = io.with_recording(writer),
            Err(error) => {
                eprintln!("cannot record in {}: {error}", settings.directory.display());
                return ExitCode::FAILURE;
            }
        }
    }
    match serve_freewheel(
        &mut io,
        FREEWHEEL_WAKE_INTERVAL,
        &AtomicBool::new(false),
        recording.as_ref(),
        keys,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("freewheel-receiver stopped: {error}");
            ExitCode::FAILURE
        }
    }
}
