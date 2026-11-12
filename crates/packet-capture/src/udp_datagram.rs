//! An IPv4 UDP datagram decoded from a captured frame.

use std::net::{Ipv4Addr, SocketAddrV4};

use crate::CapturedFrame;

const IP_PROTOCOL_UDP: u8 = 17;
const UDP_HEADER_BYTES: usize = 8;

/// An IPv4 UDP datagram decoded from a captured frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpDatagram<'a> {
    /// Capture time, in microseconds since the Unix epoch.
    pub at_us: u64,
    /// Who sent it.
    pub source: SocketAddrV4,
    /// Who it is for.
    pub destination: SocketAddrV4,
    /// UDP payload as captured: shorter than `length` when the capture truncated the frame.
    pub payload: &'a [u8],
    /// UDP payload length on the wire.
    pub length: usize,
}

impl<'a> UdpDatagram<'a> {
    /// Decodes the frame; `None` when it is not an unfragmented IPv4 UDP datagram. A frame cut
    /// short by the capture's snapshot length keeps the payload bytes that were captured.
    #[must_use]
    pub fn decode(frame: CapturedFrame<'a>) -> Option<Self> {
        let ip = frame.link_layer.ipv4_packet(frame.bytes)?;
        let ip_header_bytes = usize::from(ip.first()? & 0x0F) * 4;
        if *ip.get(9)? != IP_PROTOCOL_UDP {
            return None;
        }
        let source_ip = Ipv4Addr::new(*ip.get(12)?, ip[13], ip[14], ip[15]);
        let destination_ip = Ipv4Addr::new(*ip.get(16)?, ip[17], ip[18], ip[19]);
        let udp = ip.get(ip_header_bytes..)?;
        let source_port = u16::from_be_bytes([*udp.first()?, *udp.get(1)?]);
        let destination_port = u16::from_be_bytes([*udp.get(2)?, *udp.get(3)?]);
        let udp_length = usize::from(u16::from_be_bytes([*udp.get(4)?, *udp.get(5)?]));
        let length = udp_length.checked_sub(UDP_HEADER_BYTES)?;
        let payload = udp.get(UDP_HEADER_BYTES..udp_length.min(udp.len()))?;
        Some(Self {
            at_us: frame.at_us,
            source: SocketAddrV4::new(source_ip, source_port),
            destination: SocketAddrV4::new(destination_ip, destination_port),
            payload,
            length,
        })
    }
}
