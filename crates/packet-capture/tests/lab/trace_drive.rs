//! `trace-drive`: three recorded LTE uplinks with dead seconds, 5.3 Mbit/s together on average
//! for a 3.4 Mbit/s stream. Upstream drops the best link after a dip and does not take it back
//! while it still carries 1–3 Mbit/s, floods the 0.9 Mbit/s one instead, and most of the video
//! is lost: what our own transport must fix first.

use crate::support::{data_packets, load, modem};

/// Seconds of the run when link 1's trace (Verizon, from 240 s) carries 1–3 Mbit/s again after
/// its dip, yet upstream sends nothing on it.
const ABANDONED_FROM_S: usize = 60;
const ABANDONED_TO_S: usize = 85;
/// Full-size data packets per second that fit in link 2's 0.9 Mbit/s.
const LINK_2_PACKETS_PER_SECOND: u64 = 85;

#[test]
fn a_link_that_dipped_is_abandoned_while_it_still_works() {
    let Some(run) = load("trace-drive") else {
        return;
    };
    let on_link_1 = data_packets(&run, modem(1), ABANDONED_FROM_S, ABANDONED_TO_S);
    let on_link_2 = data_packets(&run, modem(2), ABANDONED_FROM_S, ABANDONED_TO_S);
    let seconds = u64::try_from(ABANDONED_TO_S - ABANDONED_FROM_S).unwrap_or(1);
    assert!(on_link_1 < seconds * 10, "{on_link_1} packets on link 1");
    assert!(
        on_link_2 / seconds > LINK_2_PACKETS_PER_SECOND * 3,
        "{} packets/s into a 0.9 Mbit/s link",
        on_link_2 / seconds
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
