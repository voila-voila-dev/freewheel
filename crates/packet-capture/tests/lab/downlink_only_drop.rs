//! `downlink-only-drop`: link 2 still delivers, but hears nothing back, so it is dropped.

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn a_working_uplink_is_dropped_when_its_downlink_dies() {
    let Some(run) = load("downlink-only-drop") else {
        return;
    };
    let cut_s = mark_s(&run, "cut link 2");
    let restore_s = mark_s(&run, "restore link 2");
    assert!(data_packets(&run, modem(2), cut_s, cut_s + 3) > 0);
    assert_eq!(data_packets(&run, modem(2), cut_s + 6, restore_s), 0);
}
