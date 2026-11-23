//! `freewheel-sender SRT_LISTEN_PORT RECEIVER_HOST RECEIVER_PORT BIND_IPS_FILE`: our own
//! transport (M1b), with the same command line as `bonding-sender`.
//!
//! With `FREEWHEEL_CAPACITY_SOCKET=<path>` in its environment, it also sends a capacity report
//! (`bitrate_control::CapacityReport`, JSON) to that Unix datagram socket every 100 ms, for
//! the encoder's bitrate. With `FREEWHEEL_MAX_VIDEO_BPS` (the encoder's ceiling) and
//! `FREEWHEEL_AUDIO_BPS` too, it probes the links' spare capacity while the encoder is below
//! its ceiling (`bonding_sender::freewheel::ProbeTarget`).
//!
//! With `FREEWHEEL_RECORDING=<path>`, it uploads that recording on the links' spare capacity
//! (the background class); every number of the upload is a `FREEWHEEL_RECORDING_*` variable
//! (`bonding_sender::freewheel::RecordingSettings`). Without it, no upload.

use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use bitrate_control::ControllerSettings;
use bonding_sender::freewheel::{ProbeTarget, RecordingSettings, Sender};
use bonding_sender::server::{
    CapacityOutput, FREEWHEEL_WAKE_INTERVAL, SenderConfig, UdpIo, random_session_id,
    read_source_list, serve_freewheel,
};

const CAPACITY_SOCKET_VARIABLE: &str = "FREEWHEEL_CAPACITY_SOCKET";
const MAX_VIDEO_VARIABLE: &str = "FREEWHEEL_MAX_VIDEO_BPS";
const AUDIO_VARIABLE: &str = "FREEWHEEL_AUDIO_BPS";

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
    let probe_target = probe_target()?.filter(|_| capacity.is_some());
    if let Some(target) = probe_target {
        sender.enable_probing(target);
    }
    let recording = RecordingSettings::from_environment(|name| std::env::var(name).ok())
        .map_err(|error| error.to_string())?;
    eprintln!(
        "freewheel-sender: SRT on port {}, receiver at {receiver}{}{}{}",
        config.listen_port,
        capacity_path
            .as_deref()
            .map(|path| format!(", capacity to {}", path.display()))
            .unwrap_or_default(),
        probe_target
            .map(|target| format!(", probing up to {} bit/s", target.wanted_capacity_bps))
            .unwrap_or_default(),
        recording
            .as_ref()
            .map(|settings| format!(", uploading {settings:?}"))
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

/// What the encoder's ceiling needs, from `FREEWHEEL_MAX_VIDEO_BPS` and `FREEWHEEL_AUDIO_BPS`
/// (both, or none: no probing; empty counts as none), as the encoder's controller counts it.
fn probe_target() -> Result<Option<ProbeTarget>, String> {
    let read = |name: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                value
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| format!("{name}={value}: not a bitrate in bit/s"))
            })
            .transpose()
    };
    let (Some(max_video_bps), Some(audio_bps)) = (read(MAX_VIDEO_VARIABLE)?, read(AUDIO_VARIABLE)?)
    else {
        return Ok(None);
    };
    let settings = ControllerSettings::lab(max_video_bps, audio_bps);
    Ok(Some(ProbeTarget {
        stream_max_wire_bps: settings.wire_bps(max_video_bps),
        wanted_capacity_bps: settings.capacity_for_bps(max_video_bps),
    }))
}
