//! `group-limit`: 200 fake groups fill the receiver; the real box waits until they expire.

use bonding_protocol::srtla::PacketType;

use crate::support::{data_packets, load, mark_s, modem, received};

#[test]
fn fake_groups_lock_the_box_out_until_they_expire() {
    let Some(run) = load("group-limit") else {
        return;
    };
    assert!(
        received(&run, modem(1), PacketType::RegErr as u16) > 0,
        "refused first"
    );
    assert!(
        received(&run, modem(1), PacketType::Reg3 as u16) > 0,
        "registered later"
    );
    let start_s = mark_s(&run, "sender 1 started");
    assert_eq!(data_packets(&run, modem(1), start_s, start_s + 3), 0);
    assert!(data_packets(&run, modem(1), start_s + 15, start_s + 25) > 0);
}
