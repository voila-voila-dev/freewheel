//! `burst-loss`: bursty loss on link 2 keeps pushing its share down.

use crate::support::{load, modem, share_percent};

#[test]
fn a_fading_link_keeps_losing_share() {
    let Some(run) = load("burst-loss") else {
        return;
    };
    let early = share_percent(&run, modem(2), 0, 5);
    let late = share_percent(&run, modem(2), 20, 28);
    assert!(late < early, "{early} % → {late} %");
    assert!(late <= 35, "{late} %");
    assert_eq!(run.srt.dropped_packets, 0);
}
