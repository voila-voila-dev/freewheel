//! `address-change`: modem 2 gets a new IP and the link list is reloaded (SIGHUP).

use std::net::Ipv4Addr;

use crate::support::{data_packets, load, mark_s, share_percent};

const NEW_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 3);

#[test]
fn a_new_address_starts_with_a_fresh_window() {
    let Some(run) = load("address-change") else {
        return;
    };
    let reload_s = mark_s(&run, "sender 1 reloaded");
    assert!(data_packets(&run, NEW_ADDRESS, reload_s, reload_s + 3) > 0);
    // A brand-new link starts at the default window, far above a recovering one.
    let share = share_percent(&run, NEW_ADDRESS, reload_s + 5, reload_s + 25);
    assert!((25..=60).contains(&share), "{share} %");
}
