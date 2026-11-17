//! `trace-drive`: three recorded LTE uplinks with dead seconds, 5.3 Mbit/s together on average
//! for a 3.4 Mbit/s stream. After a dip, the Verizon link (link 1) is dropped and not taken back
//! for a long stretch while its trace carries 1–3 Mbit/s again, the 0.9 Mbit/s ATT link (link 2)
//! is flooded instead, and most of the video is lost: what our own transport must fix first.
//!
//! When the stretch starts varies from run to run (44 s or 58 s locally, later in CI on
//! 2026-11-17), so the tests look for the longest one rather than at fixed seconds.

use crate::support::{data_packets, load, modem};

/// The stream runs 90 s; the first seconds are the start, not the trace.
const FROM_S: usize = 5;
const TO_S: usize = 90;
/// Under this many data packets in a second, a link counts as left aside.
const UNUSED_PACKETS_PER_SECOND: u64 = 10;
/// Shortest stretch the drop lasted in every run seen (43 s upstream, 30+ s ours locally).
const ABANDONED_AT_LEAST_S: usize = 10;
/// Full-size data packets per second that fit in link 2's 0.9 Mbit/s.
const LINK_2_PACKETS_PER_SECOND: u64 = 85;

/// The longest run of seconds in `[FROM_S, TO_S)` with link 1 left aside, as `(start, length)`.
fn longest_abandonment(run: &packet_capture::LabRun) -> (usize, usize) {
    let mut longest = (FROM_S, 0);
    let mut current: Option<usize> = None;
    for second in FROM_S..TO_S {
        let unused = data_packets(run, modem(1), second, second + 1) < UNUSED_PACKETS_PER_SECOND;
        match (unused, current) {
            (true, None) => current = Some(second),
            (false, Some(start)) => {
                if second - start > longest.1 {
                    longest = (start, second - start);
                }
                current = None;
            }
            _ => {}
        }
    }
    if let Some(start) = current
        && TO_S - start > longest.1
    {
        longest = (start, TO_S - start);
    }
    longest
}

#[test]
fn a_link_that_dipped_is_abandoned_while_it_still_works() {
    let Some(run) = load("trace-drive") else {
        return;
    };
    let (start, length) = longest_abandonment(&run);
    assert!(
        length >= ABANDONED_AT_LEAST_S,
        "link 1 left aside {length} s at most (from {start} s)"
    );
    let seconds = u64::try_from(length).unwrap_or(1);
    let flood = data_packets(&run, modem(2), start, start + length) / seconds;
    assert!(
        flood > LINK_2_PACKETS_PER_SECOND * 3,
        "{flood} packets/s into a 0.9 Mbit/s link meanwhile"
    );
}

#[test]
fn most_of_the_video_is_lost() {
    let Some(run) = load("trace-drive") else {
        return;
    };
    let Some(frames) = run.frames else { return };
    assert!(frames.damaged() > frames.sent / 3, "{frames:?}");
}
