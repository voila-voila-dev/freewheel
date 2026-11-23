//! `recording-check <recording> [<copy>]`: compares the server's copy of a recording with the
//! recording, byte for byte, and prints the lab's one-line summary
//! (`packet_capture::RecordingComparison`). Exits 0 when the copy is whole. A missing copy
//! counts as empty. Built for Linux and mounted in the lab by `labs/srtla-capture/run-all.sh`.

use std::process::ExitCode;

use packet_capture::RecordingComparison;

const USAGE: &str = "usage: recording-check <recording> [<copy>]";
const NOT_WHOLE: u8 = 1;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (recording, copy) = match arguments.as_slice() {
        [recording] => (recording, None),
        [recording, copy] => (recording, Some(copy)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let recording = match std::fs::read(recording) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("recording-check: cannot read {recording}: {error}");
            return ExitCode::from(2);
        }
    };
    let copy = copy
        .and_then(|path| std::fs::read(path).ok())
        .unwrap_or_default();
    let comparison = RecordingComparison::compare(&recording, &copy);
    println!("{}", comparison.to_line());
    if comparison.whole() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(NOT_WHOLE)
    }
}
