//! `link-drop`: a dead zone on link 2 for 10 s.

use crate::support::{data_packets, load, mark_s, modem, share_percent};

#[test]
fn upstream_keeps_sending_into_a_dead_link_for_seconds() {
    let Some(run) = load("link-drop") else { return };
    let cut_s = mark_s(&run, "cut link 2");
    assert!(data_packets(&run, modem(2), cut_s + 1, cut_s + 3) > 0);
    assert_eq!(data_packets(&run, modem(2), cut_s + 6, cut_s + 10), 0);
}

#[test]
fn a_returning_link_ramps_up_very_slowly() {
    let Some(run) = load("link-drop") else { return };
    let restore_s = mark_s(&run, "restore link 2");
    assert!(share_percent(&run, modem(2), 2, 9) >= 40);
    assert!(share_percent(&run, modem(2), restore_s + 1, restore_s + 5) <= 10);
    assert!(share_percent(&run, modem(2), restore_s + 15, restore_s + 25) <= 25);
    assert_eq!(run.srt.dropped_packets, 0);
}
