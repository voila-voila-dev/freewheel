//! Frame-by-frame comparison of sent and received video.

use std::fmt::Write as _;

use packet_capture::{FrameCheck, FrameDigests};

const HEADER: &str = "#format: frame checksums\n#version: 2\n#hash: MD5\n#tb 0: 1/90000\n#stream#, dts,        pts, duration,     size, hash\n";

fn listing(frames: &[(i64, &str)]) -> FrameDigests {
    let mut text = HEADER.to_owned();
    for (pts, hash) in frames {
        let _ = writeln!(text, "0, {pts}, {pts}, 3000, 1000, {hash}, S=1, 1, ffff");
    }
    FrameDigests::parse(&text)
}

fn sent() -> FrameDigests {
    listing(&[
        (0, "a"),
        (3000, "b"),
        (6000, "c"),
        (9000, "d"),
        (12000, "e"),
        (15000, "f"),
    ])
}

#[test]
fn parses_timestamps_hashes_and_time_base() {
    let digests = sent();
    assert_eq!(digests.time_base_hz, 90_000);
    assert_eq!(digests.frames.len(), 6);
    assert_eq!(digests.frames[1].pts, 3000);
    assert_eq!(digests.frames[1].hash, "b");
}

#[test]
fn identical_streams_are_all_intact() {
    let check = FrameCheck::compare(&sent(), &sent());
    assert_eq!(check.intact, 6);
    assert_eq!(check.damaged(), 0);
    assert_eq!(check.longest_gap_ms, 0);
    assert!((check.intact_percent() - 100.0).abs() < f64::EPSILON);
}

#[test]
fn tells_missing_corrupted_and_truncated_frames_apart() {
    let received = listing(&[(3000, "b"), (9000, "X"), (12000, "e")]);
    let check = FrameCheck::compare(&sent(), &received);
    assert_eq!(check.before_start, 1);
    assert_eq!(check.intact, 2);
    assert_eq!(check.missing, 1);
    assert_eq!(check.corrupted, 1);
    assert_eq!(check.truncated, 1);
    assert_eq!(check.damaged(), 2);
    assert_eq!(check.longest_gap_ms, 66);
}

#[test]
fn nothing_received_never_started() {
    let check = FrameCheck::compare(&sent(), &listing(&[]));
    assert_eq!(check.before_start, 6);
    assert_eq!(check.intact, 0);
    assert_eq!(check.longest_gap_ms, 0);
}

#[test]
fn received_order_does_not_matter() {
    let received = listing(&[
        (3000, "b"),
        (0, "a"),
        (9000, "d"),
        (6000, "c"),
        (15000, "f"),
        (12000, "e"),
    ]);
    assert_eq!(FrameCheck::compare(&sent(), &received).intact, 6);
}
