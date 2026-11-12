//! `burst-loss`: bursty loss on link 2 keeps pushing its share down.

use crate::support::{links_of, load, modem, share_percent};

#[test]
fn a_fading_link_keeps_losing_share() {
    let Some(run) = load("burst-loss") else {
        return;
    };
    // From link 2's first data: a bad period drawn at the very start can delay its
    // registration by seconds (1 run in 8, 2026-11-12), and its share would start at 0 %.
    let first_s = links_of(&run.box_side, modem(2))
        .filter_map(|stats| stats.data_packets_per_second.iter().position(|&n| n > 0))
        .min()
        .unwrap_or(0);
    let early = share_percent(&run, modem(2), first_s, first_s + 5);
    let late = share_percent(&run, modem(2), 20, 28);
    assert!(late < early, "{early} % → {late} %");
    assert!(late <= 35, "{late} %");
    assert_eq!(run.srt.dropped_packets, 0);
}
