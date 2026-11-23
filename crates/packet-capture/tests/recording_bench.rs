//! The recording in a lab run (`LabRun::recording`, `recording_whole_after_s`) and in
//! `bench-transport`'s table and gate, on synthetic runs (the committed fixture capture stands
//! for both captures).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use packet_capture::{LabRun, RecordingComparison};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/steady-start.pcap"
);
const SCENARIO: &str = "all-links-drop";
const STREAM_END_US: u64 = 1_795_000_000_000_000;

fn temporary(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("recording-bench-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

/// One run of `SCENARIO` in `directory`; with `recording`, its summary and marks, delivered
/// `whole_after_s` seconds after the stream ended when given.
fn write_run(directory: &Path, recording: Option<RecordingComparison>, whole_after_s: Option<u64>) {
    std::fs::create_dir_all(directory).unwrap_or_default();
    for side in ["box", "server"] {
        std::fs::copy(FIXTURE, directory.join(format!("{SCENARIO}.{side}.pcap")))
            .unwrap_or_default();
    }
    let mut marks = format!("{} scenario {SCENARIO}\n", STREAM_END_US - 35_000_000);
    if let Some(recording) = recording {
        let _ = writeln!(
            marks,
            "{STREAM_END_US} recording 1 written {} bytes",
            recording.recording_bytes
        );
        if let Some(after_s) = whole_after_s {
            let _ = writeln!(
                marks,
                "{} recording 1 delivered",
                STREAM_END_US + after_s * 1_000_000
            );
        }
        std::fs::write(
            directory.join(format!("{SCENARIO}.recording")),
            recording.to_line(),
        )
        .unwrap_or_default();
    }
    std::fs::write(directory.join(format!("{SCENARIO}.marks")), marks).unwrap_or_default();
}

const WHOLE: RecordingComparison = RecordingComparison {
    recording_bytes: 1_000,
    copy_bytes: 1_000,
    identical_bytes: 1_000,
};
const PARTIAL: RecordingComparison = RecordingComparison {
    recording_bytes: 1_000,
    copy_bytes: 1_000,
    identical_bytes: 871,
};

#[test]
fn a_run_reads_its_recording_and_the_time_to_whole() {
    let directory = temporary("run");
    write_run(&directory, Some(WHOLE), Some(12));
    let run = LabRun::load(&directory, SCENARIO);
    let _ = std::fs::remove_dir_all(&directory);
    let Ok(run) = run else {
        unreachable!("the fixture loads")
    };
    assert_eq!(run.recording, Some(WHOLE));
    assert_eq!(run.recording_whole_after_s(), Some(12.0));
}

#[test]
fn a_run_without_a_recording_has_none() {
    let directory = temporary("none");
    write_run(&directory, None, None);
    let run = LabRun::load(&directory, SCENARIO);
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        run.is_ok_and(|run| run.recording.is_none() && run.recording_whole_after_s().is_none())
    );
}

/// `bench-transport <bench> --gate transport-transport` on a baseline without a recording and
/// a candidate whose runs got `candidate` of it; returns what it printed.
fn bench(name: &str, candidate: &[Option<(RecordingComparison, Option<u64>)>]) -> String {
    let bench = temporary(name);
    write_run(&bench.join("upstream-upstream/run-1"), None, None);
    for (index, run) in candidate.iter().enumerate() {
        let directory = bench.join(format!("transport-transport/run-{}", index + 1));
        match run {
            Some((recording, after_s)) => write_run(&directory, Some(*recording), *after_s),
            None => write_run(&directory, None, None),
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_bench-transport"))
        .arg(&bench)
        .args(["--gate", "transport-transport"])
        .output();
    let _ = std::fs::remove_dir_all(&bench);
    output
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

#[test]
fn the_table_shows_the_recording_and_the_baseline_has_none() {
    let printed = bench(
        "whole",
        &[
            Some((WHOLE, Some(10))),
            Some((WHOLE, Some(14))),
            Some((WHOLE, Some(12))),
        ],
    );
    assert!(printed.contains("| 100 % in 12 s |"), "{printed}");
    assert!(printed.contains("| upstream → upstream | 1 |"), "{printed}");
    let baseline_row = printed
        .lines()
        .find(|line| line.contains("upstream → upstream"))
        .unwrap_or_default();
    assert!(baseline_row.ends_with("| – |"), "{baseline_row}");
    assert!(
        printed.contains("uploaded a recording on 1 scenarios"),
        "{printed}"
    );
    assert!(!printed.contains("recording delivered:"), "{printed}");
}

#[test]
fn one_run_short_of_the_whole_recording_fails_the_gate() {
    let printed = bench(
        "partial",
        &[
            Some((WHOLE, Some(10))),
            Some((PARTIAL, None)),
            Some((WHOLE, Some(12))),
        ],
    );
    assert!(
        printed.contains("- all-links-drop, recording delivered: 87.1 % (2/3 runs whole)"),
        "{printed}"
    );
}

#[test]
fn without_a_recording_there_is_no_recording_gate() {
    let printed = bench("off", &[None, None]);
    assert!(printed.contains("uploaded no recording"), "{printed}");
    assert!(!printed.contains("recording delivered:"), "{printed}");
}
