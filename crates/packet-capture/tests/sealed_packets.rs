//! Our transport's sealed packets (wire format v1, M1b step 7) are counted from their clear
//! header: a live data packet counts on its link, anything else of ours does not.

use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::freewheel::{
    Data, Header, MAX_DATAGRAM_LEN, Packet, TrafficClass, frame_sealed,
};
use packet_capture::{SrtlaReport, UdpDatagram};

const RECEIVER_PORT: u16 = 5_000;
const LINK: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 1, 0, 2), 40_000);
const RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), RECEIVER_PORT);

/// A data packet of `class`, laid out sealed (the encrypted part left in clear: the report
/// reads only the header).
fn sealed(class: TrafficClass) -> Vec<u8> {
    let payload = [0u8; 1_316];
    let packet = Packet::Data(Data {
        header: Header {
            session_id: 7,
            link_id: 0,
        },
        class,
        sequence: 1,
        sent_at_us: 0,
        payload: &payload,
    });
    let mut plain = [0u8; MAX_DATAGRAM_LEN];
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let Some(len) = packet
        .encode(&mut plain)
        .ok()
        .and_then(|len| frame_sealed(&plain[..len], 0, &mut out).ok())
    else {
        unreachable!("a 1316-byte payload fits sealed")
    };
    out[..len].to_vec()
}

#[test]
fn a_sealed_live_packet_counts_as_data_and_a_sealed_recording_chunk_does_not() {
    let live = sealed(TrafficClass::Live);
    let background = sealed(TrafficClass::Background);
    let datagrams: Vec<UdpDatagram<'_>> = [&live, &background, &live]
        .iter()
        .enumerate()
        .map(|(index, payload)| UdpDatagram {
            at_us: 1_000 * index as u64,
            source: LINK,
            destination: RECEIVER,
            payload,
            length: payload.len(),
        })
        .collect();
    let report = SrtlaReport::build(RECEIVER_PORT, &datagrams);
    let stats = report.links.get(&LINK);
    assert_eq!(stats.map(|stats| stats.data_packets), Some(2));
    assert_eq!(
        stats.map(|stats| stats.data_bytes),
        Some(2 * live.len() as u64)
    );
}
