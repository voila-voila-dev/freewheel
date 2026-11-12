//! `link-removed`: a modem removed from the list stops carrying traffic at once.

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn a_removed_modem_stops_at_once() {
    let Some(run) = load("link-removed") else {
        return;
    };
    let reload_s = mark_s(&run, "sender 1 reloaded");
    assert_eq!(data_packets(&run, modem(2), reload_s + 2, reload_s + 20), 0);
    assert!(data_packets(&run, modem(1), reload_s + 2, reload_s + 20) > 0);
    assert_eq!(run.srt.dropped_packets, 0);
}
