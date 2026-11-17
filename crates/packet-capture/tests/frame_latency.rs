//! `FrameLatency`: encoder-to-server latency over the frames that arrived intact.

use std::collections::HashSet;

use packet_capture::{FrameLatency, FrameTime};

fn frames(times: &[(i64, u64)]) -> Vec<FrameTime> {
    times
        .iter()
        .map(|&(pts, at_us)| FrameTime { pts, at_us })
        .collect()
}

#[test]
fn only_intact_frames_count() {
    let sent = frames(&[(1, 0), (2, 1_000), (3, 2_000), (4, 3_000)]);
    let received = frames(&[(1, 100_000), (2, 201_000), (3, 5_002_000), (4, 303_000)]);
    let intact: HashSet<i64> = [1, 2, 4].into();
    let latency = FrameLatency::measure(&sent, &received, &intact);
    assert_eq!(
        latency,
        Some(FrameLatency {
            frames: 3,
            median_ms: 200.0,
            p99_ms: 300.0,
            max_ms: 300.0
        })
    );
}

#[test]
fn a_frame_delivered_twice_counts_once() {
    let sent = frames(&[(1, 0)]);
    let received = frames(&[(1, 50_000), (1, 900_000)]);
    let latency = FrameLatency::measure(&sent, &received, &[1].into());
    assert_eq!(latency.map(|latency| latency.max_ms), Some(50.0));
}

#[test]
fn nothing_to_measure_without_a_match() {
    assert_eq!(
        FrameLatency::measure(&frames(&[(1, 0)]), &[], &[1].into()),
        None
    );
}
