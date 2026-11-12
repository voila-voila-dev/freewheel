//! `long-run`: ten minutes on two links. Nothing leaks, drifts or stalls.

use crate::support::{data_packets, load, modem};

const RUN_S: usize = 600;
const MEMORY_GROWTH_LIMIT_KB: u64 = 2_048;

fn memory_kb(run: &packet_capture::LabRun, process: &str) -> Vec<u64> {
    let prefix = format!("memory {process} ");
    run.marks
        .iter()
        .filter_map(|mark| mark.label.strip_prefix(&prefix)?.parse().ok())
        .collect()
}

#[test]
fn memory_stays_flat() {
    let Some(run) = load("long-run") else { return };
    for process in ["sender", "receiver"] {
        let samples = memory_kb(&run, process);
        assert!(samples.len() >= 10, "{process}: {samples:?}");
        let first = samples.first().copied().unwrap_or_default();
        let last = samples.last().copied().unwrap_or_default();
        assert!(first > 0, "{process} was running");
        assert!(
            last <= first + MEMORY_GROWTH_LIMIT_KB,
            "{process}: {samples:?} kB"
        );
    }
}

#[test]
fn the_stream_never_stalls() {
    let Some(run) = load("long-run") else { return };
    for second in 2..RUN_S - 2 {
        let sent = data_packets(&run, modem(1), second, second + 1)
            + data_packets(&run, modem(2), second, second + 1);
        assert!(sent > 0, "nothing sent at {second} s");
    }
    assert!(run.sender_exits().iter().all(|(_, code)| *code == 143));
    assert_eq!(run.srt.dropped_packets, 0);
}
