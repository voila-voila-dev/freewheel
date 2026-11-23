//! The recording against its copy (`RecordingComparison`), and the lab's summary line.

use packet_capture::RecordingComparison;

#[test]
fn an_exact_copy_is_whole() {
    let recording: Vec<u8> = (1..=200).collect();
    let comparison = RecordingComparison::compare(&recording, &recording);
    assert!(comparison.whole());
    assert_eq!(comparison.delivered_percent(), Some(100.0));
}

#[test]
fn a_hole_or_a_short_copy_is_not() {
    let recording: Vec<u8> = (1..=200).collect();
    let mut holed = recording.clone();
    holed[50..60].fill(0);
    let comparison = RecordingComparison::compare(&recording, &holed);
    assert_eq!(comparison.identical_bytes, 190);
    assert!(!comparison.whole());
    assert_eq!(comparison.delivered_percent(), Some(95.0));
    let short = RecordingComparison::compare(&recording, &recording[..100]);
    assert_eq!(short.delivered_percent(), Some(50.0));
    let almost = RecordingComparison::compare(&vec![1; 10_000], &vec![1; 9_999]);
    assert_eq!(
        almost.delivered_percent(),
        Some(99.9),
        "never 100 unless whole"
    );
    assert_eq!(
        RecordingComparison::compare(&[], &[]).delivered_percent(),
        None
    );
}

#[test]
fn the_summary_line_reads_back() {
    let comparison = RecordingComparison {
        recording_bytes: 30,
        copy_bytes: 20,
        identical_bytes: 19,
    };
    assert_eq!(
        comparison.to_line(),
        "recording_bytes=30 copy_bytes=20 identical_bytes=19"
    );
    assert_eq!(
        RecordingComparison::parse(&format!("{}\n", comparison.to_line())),
        Some(comparison)
    );
    assert_eq!(RecordingComparison::parse("recording_bytes=3"), None);
}
