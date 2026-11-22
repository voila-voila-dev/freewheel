//! `lab-encoder --width W --height H --fps F --keyint K --max-bitrate BPS [--audio-bitrate BPS]
//! [--capacity-socket PATH]`
//!
//! The lab's adaptive encoder (M1b step 5): raw I420 frames on stdin, H.264 (Annex B) on
//! stdout. With `--capacity-socket`, it listens there for the transport's capacity reports
//! (`bitrate_control::CapacityReport`, sent by `freewheel-sender`) and follows
//! `bitrate_control::Controller` before each frame, counting `--audio-bitrate` (0 by default)
//! as the rest of the stream; without it, it stays at `--max-bitrate`.
//! Every bitrate it takes is written to stderr as a lab mark: `<µs since the Unix epoch>
//! encoder bitrate <bit/s>`.

use std::io::{self, BufWriter, Read, Write};
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bitrate_control::{CapacityReport, Controller, ControllerSettings, MAX_REPORT_LEN};
use h264_encoder::{Encoder, EncoderSettings};

const USAGE: &str = "usage: lab-encoder --width W --height H --fps F --keyint K --max-bitrate BPS [--audio-bitrate BPS] [--capacity-socket PATH]";
/// Set in the environment, every capacity report is a mark too (`encoder report …`).
const LOG_REPORTS_VARIABLE: &str = "LAB_ENCODER_LOG_REPORTS";
/// Room for one encoded frame: a keyframe of the lab's 720p test pattern is ~40 kB at 3 Mbit/s.
const OUTPUT_BYTES: usize = 4 * 1024 * 1024;

struct Options {
    settings: EncoderSettings,
    audio_bps: u32,
    capacity_socket: Option<PathBuf>,
}

fn main() -> ExitCode {
    let Some(options) = parse_options(std::env::args().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("lab-encoder: {message}");
            ExitCode::FAILURE
        }
    }
}

fn parse_options(arguments: Vec<String>) -> Option<Options> {
    let mut arguments = arguments.into_iter();
    let (mut width, mut height, mut fps, mut keyint, mut max_bps) = (None, None, None, None, None);
    let mut capacity_socket = None;
    let mut audio_bps = Some(0);
    while let Some(flag) = arguments.next() {
        let value = arguments.next()?;
        match flag.as_str() {
            "--width" => width = value.parse().ok(),
            "--height" => height = value.parse().ok(),
            "--fps" => fps = value.parse().ok(),
            "--keyint" => keyint = value.parse().ok(),
            "--max-bitrate" => max_bps = value.parse().ok(),
            "--audio-bitrate" => audio_bps = value.parse().ok(),
            "--capacity-socket" => capacity_socket = Some(PathBuf::from(value)),
            _ => return None,
        }
    }
    Some(Options {
        settings: EncoderSettings {
            width: width?,
            height: height?,
            fps: fps?,
            keyint: keyint?,
            bitrate_bps: max_bps?,
        },
        audio_bps: audio_bps?,
        capacity_socket,
    })
}

fn run(options: &Options) -> Result<(), String> {
    let mut encoder = Encoder::open(options.settings).map_err(|error| error.to_string())?;
    let reports = options
        .capacity_socket
        .as_ref()
        .map(|path| {
            let _ = std::fs::remove_file(path);
            let socket = UnixDatagram::bind(path)
                .map_err(|error| format!("cannot listen on {}: {error}", path.display()))?;
            socket
                .set_nonblocking(true)
                .map_err(|error| format!("cannot set {} non-blocking: {error}", path.display()))?;
            Ok::<_, String>(socket)
        })
        .transpose()?;
    let mut controller = Controller::new(ControllerSettings::lab(
        options.settings.bitrate_bps,
        options.audio_bps,
    ));
    let started = Instant::now();
    let log_reports = std::env::var_os(LOG_REPORTS_VARIABLE).is_some();
    log_bitrate(encoder.bitrate_bps());

    let mut frame = vec![0u8; options.settings.frame_bytes()];
    let mut out = vec![0u8; OUTPUT_BYTES];
    let mut report = [0u8; MAX_REPORT_LEN];
    let mut stdin = io::stdin().lock();
    let mut stdout = BufWriter::new(io::stdout().lock());
    let mut pts: i64 = 0;
    loop {
        match stdin.read_exact(&mut frame) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(format!("cannot read a frame: {error}")),
        }
        if let Some(socket) = &reports {
            let now_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            while let Ok(len) = socket.recv(&mut report) {
                if let Some(capacity) = report.get(..len).and_then(CapacityReport::decode) {
                    controller.on_report(now_ms, &capacity);
                    if log_reports {
                        log_report(&capacity);
                    }
                }
            }
            let wanted = controller.bitrate_bps();
            if wanted != encoder.bitrate_bps() {
                encoder
                    .set_bitrate_bps(wanted)
                    .map_err(|error| format!("cannot set {wanted} bit/s: {error}"))?;
                log_bitrate(wanted);
            }
        }
        let bytes = encoder
            .encode(&frame, pts, &mut out)
            .map_err(|error| format!("frame {pts}: {error}"))?;
        pts += 1;
        let h264 = out.get(..bytes).unwrap_or_default();
        stdout
            .write_all(h264)
            .and_then(|()| stdout.flush())
            .map_err(|error| format!("cannot write: {error}"))?;
    }
    Ok(())
}

/// A capacity report as a lab mark, on stderr.
fn log_report(report: &CapacityReport) {
    eprintln!(
        "{} encoder report capacity={} sent={} overflow={} dropped={} links={}",
        unix_us(),
        report.capacity_bps,
        report.sent_bps,
        report.overflow_bps,
        report.dropped_bps,
        report.usable_links
    );
}

fn unix_us() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_micros())
}

/// A lab mark on stderr: the lab appends the encoder's stderr to `<scenario>.marks`.
fn log_bitrate(bitrate_bps: u32) {
    eprintln!("{} encoder bitrate {bitrate_bps}", unix_us());
}
