//! `two-senders`: two boxes, two links each, on one receiver.

use bonding_protocol::srtla::PacketType;

use crate::support::{load, modem, received};

#[test]
fn two_boxes_get_two_groups() {
    let Some(run) = load("two-senders") else {
        return;
    };
    assert_eq!(run.server_side.group_ids.len(), 2);
    for k in 1..=4 {
        assert!(
            received(&run, modem(k), PacketType::Reg3 as u16) > 0,
            "link {k}"
        );
    }
}
