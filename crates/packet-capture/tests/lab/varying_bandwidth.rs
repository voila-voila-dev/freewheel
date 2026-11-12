//! `varying-bandwidth`: link 2's capacity moves between 8, 1, 4 and 0.5 Mbit/s.
//!
//! Thresholds hold over 8 runs (2026-11-11, 4 with upstream's receiver, 4 with ours): the
//! overflow ranged 73–159 packets/s into the 1 Mbit/s link, retransmissions 1 099–2 503, and the
//! share 12–20 s after the capacity returned 40–74 %.

use crate::support::{data_packets, load, mark_s, modem, share_percent};

/// Full-size data packets per second that fit in 1 Mbit/s.
const PACKETS_PER_SECOND_AT_1_MBIT: u64 = 90;
const PHASE_S: usize = 8;

#[test]
fn a_link_that_slows_down_keeps_its_traffic_until_losses_pile_up() {
    let Some(run) = load("varying-bandwidth") else {
        return;
    };
    let slow_s = mark_s(&run, "link 2 capacity 1mbit");
    let sent = data_packets(&run, modem(2), slow_s + 1, slow_s + PHASE_S);
    let seconds = u64::try_from(PHASE_S - 1).unwrap_or(1);
    assert!(
        sent / seconds > PACKETS_PER_SECOND_AT_1_MBIT * 2 / 3,
        "{} packets/s into a 1 Mbit/s link",
        sent / seconds
    );
    assert!(
        run.srt.retransmitted_packets > 800,
        "the overflow is paid in retransmissions: {}",
        run.srt.retransmitted_packets
    );
    assert_eq!(run.srt.dropped_packets, 0);
}

#[test]
fn a_link_that_only_slowed_down_gets_its_share_back() {
    let Some(run) = load("varying-bandwidth") else {
        return;
    };
    // Unlike a link that died (`link-drop`, ~15 % 25 s later), one that only slowed down is
    // back near a fair share 12–20 s after its capacity returns.
    let back_s = mark_s(&run, "link 2 capacity 500kbit") + PHASE_S;
    let share = share_percent(&run, modem(2), back_s + 12, back_s + 20);
    assert!(share >= 35, "{share} %");
}
