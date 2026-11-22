//! The session's window of received sequences: duplicates dropped, holes reported in the
//! ACK ranges below a link's last sequence, across the `u32` wrap.

use bonding_protocol::freewheel::SequenceRange;
use bonding_receiver::freewheel::{ACK_RANGES, Arrival, RECEIVED_WINDOW_SEQUENCES, ReceivedWindow};

fn ranges_below(window: &ReceivedWindow, from: u32) -> Vec<(u32, u32)> {
    let mut out = [SequenceRange::default(); ACK_RANGES];
    let count = window.ranges_below(from, &mut out);
    out[..count].iter().map(|r| (r.first, r.last)).collect()
}

#[test]
fn a_second_copy_is_a_duplicate() {
    let mut window = ReceivedWindow::new();
    assert_eq!(window.record(10), Arrival::New);
    assert_eq!(window.record(12), Arrival::New);
    assert_eq!(window.record(11), Arrival::New);
    assert_eq!(window.record(11), Arrival::Duplicate);
    assert_eq!(window.record(10), Arrival::Duplicate);
}

#[test]
fn too_old_to_tell_is_forwarded() {
    let mut window = ReceivedWindow::new();
    window.record(5_000);
    assert_eq!(
        window.record(5_000 - RECEIVED_WINDOW_SEQUENCES),
        Arrival::New
    );
}

#[test]
fn holes_split_the_ranges_oldest_first() {
    let mut window = ReceivedWindow::new();
    for sequence in [0, 1, 2, 4, 5, 8, 9] {
        window.record(sequence);
    }
    assert_eq!(ranges_below(&window, 9), [(0, 2), (4, 5), (8, 9)]);
    assert_eq!(ranges_below(&window, 5), [(0, 2), (4, 5)]);
}

#[test]
fn a_jump_forgets_what_it_skipped() {
    let mut window = ReceivedWindow::new();
    window.record(3);
    window.record(3 + RECEIVED_WINDOW_SEQUENCES + 10);
    assert_eq!(
        window.record(3 + RECEIVED_WINDOW_SEQUENCES),
        Arrival::New,
        "a slot reused by the jump is not a duplicate"
    );
}

#[test]
fn ranges_follow_the_wrap() {
    let mut window = ReceivedWindow::new();
    for sequence in [u32::MAX - 2, u32::MAX - 1, 1, 2] {
        window.record(sequence);
    }
    assert_eq!(
        ranges_below(&window, 2),
        [(u32::MAX - 2, u32::MAX - 1), (1, 2)]
    );
}

#[test]
fn at_most_the_newest_ranges_fit() {
    let mut window = ReceivedWindow::new();
    let top = 2 * u32::try_from(ACK_RANGES).unwrap_or(0) + 20;
    for sequence in (0..=top).step_by(2) {
        window.record(sequence);
    }
    let ranges = ranges_below(&window, top);
    assert_eq!(ranges.len(), ACK_RANGES);
    assert_eq!(ranges.last(), Some(&(top, top)));
}

#[test]
fn nothing_below_a_sequence_that_left_the_window() {
    let mut window = ReceivedWindow::new();
    window.record(1);
    window.record(1 + RECEIVED_WINDOW_SEQUENCES);
    assert!(ranges_below(&window, 1).is_empty());
}
