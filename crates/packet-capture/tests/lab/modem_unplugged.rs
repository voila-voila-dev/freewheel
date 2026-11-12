//! `modem-unplugged`: a failed send takes the link out at once, without loss.

use crate::support::{data_packets, load, mark_s, modem, share_percent};

#[test]
fn a_vanished_modem_is_dropped_at_once() {
    let Some(run) = load("modem-unplugged") else {
        return;
    };
    let unplug_s = mark_s(&run, "unplug modem 2");
    let plug_s = mark_s(&run, "plug modem 2");
    assert_eq!(data_packets(&run, modem(2), unplug_s + 1, plug_s), 0);
    assert!(share_percent(&run, modem(2), plug_s + 5, plug_s + 15) <= 15);
    assert_eq!(run.srt.dropped_packets, 0);
}
