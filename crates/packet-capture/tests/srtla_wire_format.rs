//! Every SRTLA datagram of a real upstream capture decodes, and encodes back byte for byte.

use std::collections::BTreeSet;

use bonding_protocol::srtla::{Packet, PacketType};
use packet_capture::{LAB_RECEIVER_PORT as RECEIVER_PORT, PcapFile, UdpDatagram};

const FIXTURE: &[u8] = include_bytes!("fixtures/steady-start.pcap");

#[test]
fn every_captured_datagram_decodes_and_encodes_back() {
    let Ok(file) = PcapFile::parse(FIXTURE) else {
        unreachable!("fixture is a valid pcap file")
    };
    let Ok(frames) = file.frames() else {
        unreachable!("fixture is complete")
    };
    let mut kinds = BTreeSet::new();
    let mut out = [0u8; bonding_protocol::srtla::MAX_DATAGRAM_LEN];
    for datagram in frames.into_iter().filter_map(UdpDatagram::decode) {
        if datagram.source.port() != RECEIVER_PORT && datagram.destination.port() != RECEIVER_PORT {
            continue;
        }
        let packet = Packet::decode(datagram.payload);
        let Ok(packet) = packet else {
            unreachable!("{datagram:?}: {packet:?}");
        };
        let Ok(len) = packet.encode(&mut out) else {
            unreachable!("{packet:?} does not encode");
        };
        assert_eq!(&out[..len], datagram.payload);
        kinds.insert(packet.packet_type().map(PacketType::raw));
    }
    let expected = [
        None,
        Some(PacketType::SrtHandshake.raw()),
        Some(PacketType::SrtAck.raw()),
        Some(PacketType::SrtlaAck.raw()),
        Some(PacketType::Reg1.raw()),
        Some(PacketType::Reg2.raw()),
        Some(PacketType::Reg3.raw()),
        Some(PacketType::RegNgp.raw()),
    ];
    for kind in expected {
        assert!(kinds.contains(&kind), "{kind:?} missing from the fixture");
    }
}
