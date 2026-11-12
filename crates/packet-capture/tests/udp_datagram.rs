//! Decoding UDP datagrams, including frames cut short by the capture's snapshot length.

use std::net::{Ipv4Addr, SocketAddrV4};

use packet_capture::{CapturedFrame, LinkLayer, UdpDatagram};

const IP_HEADER: [u8; 20] = [
    0x45, 0, 0, 0, 0, 0, 0, 0, 64, 17, 0, 0, 10, 1, 0, 2, 10, 200, 0, 1,
];
const PAYLOAD_BYTES: u16 = 100;

fn frame() -> Vec<u8> {
    let mut frame = vec![0u8; 12];
    frame.extend_from_slice(&[0x08, 0x00]);
    frame.extend_from_slice(&IP_HEADER);
    frame.extend_from_slice(&6001u16.to_be_bytes());
    frame.extend_from_slice(&5000u16.to_be_bytes());
    frame.extend_from_slice(&(8 + PAYLOAD_BYTES).to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend(std::iter::repeat_n(0xAB, usize::from(PAYLOAD_BYTES)));
    frame
}

fn decode(bytes: &[u8]) -> Option<UdpDatagram<'_>> {
    UdpDatagram::decode(CapturedFrame {
        at_us: 0,
        link_layer: LinkLayer::Ethernet,
        bytes,
    })
}

#[test]
fn decodes_addresses_and_payload() {
    let bytes = frame();
    let datagram = decode(&bytes);
    assert_eq!(
        datagram.map(|d| (d.source, d.destination, d.payload.len(), d.length)),
        Some((
            SocketAddrV4::new(Ipv4Addr::new(10, 1, 0, 2), 6001),
            SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), 5000),
            100,
            100
        ))
    );
}

#[test]
fn a_truncated_frame_keeps_its_captured_bytes_and_its_wire_length() {
    let bytes = frame();
    let cut = &bytes[..bytes.len() - 60];
    assert_eq!(
        decode(cut).map(|d| (d.payload.len(), d.length)),
        Some((40, 100))
    );
}
