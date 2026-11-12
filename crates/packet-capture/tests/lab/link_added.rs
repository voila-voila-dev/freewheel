//! `link-added`: a third modem plugged in mid-stream takes its share within seconds.

use crate::support::{load, mark_s, modem, share_percent};

#[test]
fn a_new_modem_takes_traffic_quickly() {
    let Some(run) = load("link-added") else {
        return;
    };
    let reload_s = mark_s(&run, "sender 1 reloaded");
    assert_eq!(share_percent(&run, modem(3), 0, reload_s), 0);
    assert!(share_percent(&run, modem(3), reload_s + 3, reload_s + 20) >= 20);
}
