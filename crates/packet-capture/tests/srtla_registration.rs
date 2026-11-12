//! What upstream SRTLA does at start-up, checked on a real capture (`fixtures/steady-start.pcap`).

mod support;

use bonding_protocol::srtla::PacketType;
use support::report_of;

const FIXTURE: &[u8] = include_bytes!("fixtures/steady-start.pcap");

#[test]
fn registration_follows_reg2_ngp_reg1_reg2_reg3() {
    let report = report_of(FIXTURE);
    let first_link = report
        .timeline
        .iter()
        .find(|e| e.kind == PacketType::Reg1)
        .map(|e| e.link);
    let exchange: Vec<(bool, PacketType)> = report
        .timeline
        .iter()
        .filter(|e| Some(e.link) == first_link && e.kind != PacketType::SrtHandshake)
        .map(|e| (e.uplink, e.kind))
        .collect();
    assert_eq!(
        exchange,
        [
            (true, PacketType::Reg2),
            (false, PacketType::RegNgp),
            (true, PacketType::Reg1),
            (false, PacketType::Reg2),
            (true, PacketType::Reg2),
            (false, PacketType::Reg3),
        ]
    );
}

#[test]
fn receiver_keeps_the_sender_half_of_the_group_id() {
    assert_eq!(report_of(FIXTURE).reg2_keeps_sender_half, [true]);
}

#[test]
fn every_link_gets_its_own_reg3() {
    let report = report_of(FIXTURE);
    for stats in report.links.values() {
        assert_eq!(
            stats.downlink_control.get(&(PacketType::Reg3 as u16)),
            Some(&1)
        );
    }
}
