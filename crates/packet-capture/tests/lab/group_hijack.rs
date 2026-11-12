//! `group-hijack`: the group id is the only credential; whoever sniffs it joins the group.

use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::srtla::PacketType;

use crate::support::load;

const STRANGER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 3, 0, 2), 42000);
const INJECTED_PACKETS: u64 = 50;

#[test]
fn a_sniffed_group_id_lets_a_stranger_join_and_inject() {
    let Some(run) = load("group-hijack") else {
        return;
    };
    let stranger = run.box_side.links.get(&STRANGER);
    let received = |kind: PacketType| {
        stranger
            .and_then(|stats| stats.downlink_control.get(&(kind as u16)))
            .copied()
            .unwrap_or_default()
    };
    assert_eq!(received(PacketType::Reg3), 1, "the stranger is accepted");
    assert!(
        received(PacketType::SrtAck) > 100,
        "the stream's SRT ACKs leak to the stranger"
    );
    assert_eq!(
        stranger.map(|stats| stats.srtla_acked),
        Some(INJECTED_PACKETS),
        "its injected packets are acknowledged, so forwarded to the SRT server"
    );
}
