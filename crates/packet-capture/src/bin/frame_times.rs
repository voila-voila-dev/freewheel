//! When each video frame went by, on either end of the lab, as `<pts> <unix µs>` lines.
//!
//! ```text
//! frame-times record <ts file> <times file>   copies stdin to the end of <ts file> and appends
//!                                             one line per frame it completes to <times file>
//! frame-times pcap <pcap file> <udp port>     prints one line per frame of the MPEG-TS
//!                                             datagrams sent to <udp port> in the capture
//! ```
//!
//! `record` sits behind the SRT listener (what the server delivered, timed as it is written);
//! `pcap` reads a capture of the encoder's UDP output (what the box sent). Both use the host's
//! clock, so the difference per frame is the latency. Built for Linux and mounted in the lab by
//! `labs/srtla-capture/run-all.sh`.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use packet_capture::{FrameClock, FrameTime, PcapFile, UdpDatagram};

const USAGE: &str =
    "usage: frame-times record <ts file> <times file> | frame-times pcap <pcap file> <udp port>";
/// One read from the SRT listener: it writes a few 1316-byte payloads at a time.
const READ_BUFFER_BYTES: usize = 64 * 1024;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [command, video, times] if command == "record" => record(video, times),
        [command, capture, port] if command == "pcap" => match port.parse() {
            Ok(port) => from_capture(capture, port),
            Err(_) => Err(format!("not a UDP port: {port}")),
        },
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("frame-times: {message}");
            ExitCode::FAILURE
        }
    }
}

/// stdin → `video` (appended), frame times → `times` (appended), until stdin ends.
fn record(video: &str, times: &str) -> Result<(), String> {
    let append = |path: &str| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| format!("cannot open {path}: {error}"))
    };
    let mut video_file = append(video)?;
    let mut times_file = BufWriter::new(append(times)?);
    let mut clock = FrameClock::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    let mut stdin = std::io::stdin().lock();
    let mut failure = None;
    let mut write_line = |frame: FrameTime| {
        if failure.is_none()
            && let Err(error) = writeln!(times_file, "{}", frame.line())
        {
            failure = Some(format!("cannot write {times}: {error}"));
        }
    };
    loop {
        let read = match stdin.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("cannot read stdin: {error}")),
        };
        let at_us = now_us();
        // The video first: a frame time must never point at bytes not yet on disk.
        video_file
            .write_all(&buffer[..read])
            .map_err(|error| format!("cannot write {video}: {error}"))?;
        clock.push(at_us, &buffer[..read], &mut write_line);
    }
    clock.finish(&mut write_line);
    if let Some(message) = failure {
        return Err(message);
    }
    times_file
        .flush()
        .map_err(|error| format!("cannot write {times}: {error}"))
}

/// Frame times of the MPEG-TS datagrams sent to `port` in a capture, on stdout.
fn from_capture(path: &str, port: u16) -> Result<(), String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| format!("cannot read {path}: {error}"))?;
    let frames = PcapFile::parse(&bytes)
        .and_then(|file| file.frames())
        .map_err(|error| format!("{path}: {error}"))?;
    let mut lines = String::new();
    let mut add_line = |frame: FrameTime| {
        lines.push_str(&frame.line());
        lines.push('\n');
    };
    let mut clock = FrameClock::new();
    for datagram in frames.into_iter().filter_map(UdpDatagram::decode) {
        if datagram.destination.port() == port {
            clock.push(datagram.at_us, datagram.payload, &mut add_line);
        }
    }
    clock.finish(&mut add_line);
    std::io::stdout()
        .lock()
        .write_all(lines.as_bytes())
        .map_err(|error| format!("cannot write stdout: {error}"))
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_micros()).unwrap_or(u64::MAX)
        })
}
