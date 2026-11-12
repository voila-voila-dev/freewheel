//! `uplink-only-drop`: only the uplink of link 2 dies; broadcast SRT ACKs keep it "alive".

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn a_one_way_dead_link_is_never_detected() {
    let Some(run) = load("uplink-only-drop") else {
        return;
    };
    let cut_s = mark_s(&run, "cut link 2");
    let restore_s = mark_s(&run, "restore link 2");
    // SRT ACKs still arrive on link 2 (they are sent on every link), so it never times out:
    // the box sends into it for the whole cut.
    for second in cut_s + 2..restore_s {
        assert!(
            data_packets(&run, modem(2), second, second + 1) > 0,
            "second {second}"
        );
    }
}
