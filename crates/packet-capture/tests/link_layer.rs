//! Finding the IPv4 packet behind each supported link-layer header.

use packet_capture::LinkLayer;

const IPV4: [u8; 2] = [0x08, 0x00];
const IPV6: [u8; 2] = [0x86, 0xDD];
const IP_PACKET: [u8; 4] = [0x45, 0, 0, 20];

fn frame(header_bytes: usize, protocol_offset: usize, protocol: [u8; 2]) -> Vec<u8> {
    let mut frame = vec![0u8; header_bytes];
    frame[protocol_offset..protocol_offset + 2].copy_from_slice(&protocol);
    frame.extend_from_slice(&IP_PACKET);
    frame
}

#[test]
fn every_supported_header_leads_to_the_ip_packet() {
    let cases = [
        (LinkLayer::Ethernet, 1, 14, 12),
        (LinkLayer::LinuxCooked, 113, 16, 14),
        (LinkLayer::LinuxCookedV2, 276, 20, 0),
    ];
    for (layer, link_type, header_bytes, protocol_offset) in cases {
        assert_eq!(LinkLayer::from_link_type(link_type), Some(layer));
        let ipv4 = frame(header_bytes, protocol_offset, IPV4);
        assert_eq!(layer.ipv4_packet(&ipv4), Some(&IP_PACKET[..]), "{layer:?}");
        let ipv6 = frame(header_bytes, protocol_offset, IPV6);
        assert_eq!(layer.ipv4_packet(&ipv6), None, "{layer:?} ignores IPv6");
        assert_eq!(
            layer.ipv4_packet(&ipv4[..header_bytes - 1]),
            None,
            "{layer:?} truncated"
        );
    }
    assert_eq!(
        LinkLayer::from_link_type(105),
        None,
        "802.11 is not supported"
    );
}
