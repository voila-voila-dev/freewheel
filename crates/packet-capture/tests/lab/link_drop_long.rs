//! `link-drop-long`: 20 s cut, longer than the receiver's memory of the link.

use bonding_protocol::srtla::PacketType;

use crate::support::{load, mark_s, modem, received, sent, share_percent};

#[test]
fn the_link_has_to_register_again() {
    let Some(run) = load("link-drop-long") else {
        return;
    };
    // Once at start, once after the cut; a REG2 retried just as the link returns can be
    // answered too. Upstream itself got 4 in 1 of 8 runs (2026-11-12), so not exactly 2.
    let registrations = received(&run, modem(2), PacketType::Reg3 as u16);
    assert!((2..=4).contains(&registrations), "{registrations} REG3");
    assert!(
        sent(&run, modem(2), PacketType::Reg2 as u16) >= 10,
        "REG2 every second while down"
    );
    let restore_s = mark_s(&run, "restore link 2");
    assert!(share_percent(&run, modem(2), restore_s + 5, restore_s + 14) <= 15);
}
