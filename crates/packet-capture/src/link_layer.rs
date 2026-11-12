//! The link-layer header in front of each captured frame, and where the IP packet starts.

const ETHERNET_HEADER_BYTES: usize = 14;
const ETHERNET_TYPE_OFFSET: usize = 12;
const LINUX_COOKED_HEADER_BYTES: usize = 16;
const LINUX_COOKED_PROTOCOL_OFFSET: usize = 14;
const LINUX_COOKED_V2_HEADER_BYTES: usize = 20;
const LINUX_COOKED_V2_PROTOCOL_OFFSET: usize = 0;
const ETHER_TYPE_IPV4: u16 = 0x0800;

/// The link-layer header in front of each captured frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkLayer {
    /// Ethernet (pcap link type 1): one interface.
    Ethernet,
    /// Linux cooked capture (link type 113): `tcpdump -i any`.
    LinuxCooked,
    /// Linux cooked capture v2 (link type 276): `tcpdump -i any` on newer systems.
    LinuxCookedV2,
}

impl LinkLayer {
    /// Maps a pcap link type to a supported link layer.
    #[must_use]
    pub const fn from_link_type(link_type: u32) -> Option<Self> {
        match link_type {
            1 => Some(Self::Ethernet),
            113 => Some(Self::LinuxCooked),
            276 => Some(Self::LinuxCookedV2),
            _ => None,
        }
    }

    /// The IPv4 packet inside a frame; `None` for anything else.
    #[must_use]
    pub fn ipv4_packet(self, frame: &[u8]) -> Option<&[u8]> {
        let (protocol_offset, header_bytes) = match self {
            Self::Ethernet => (ETHERNET_TYPE_OFFSET, ETHERNET_HEADER_BYTES),
            Self::LinuxCooked => (LINUX_COOKED_PROTOCOL_OFFSET, LINUX_COOKED_HEADER_BYTES),
            Self::LinuxCookedV2 => (
                LINUX_COOKED_V2_PROTOCOL_OFFSET,
                LINUX_COOKED_V2_HEADER_BYTES,
            ),
        };
        let protocol = u16::from_be_bytes([
            *frame.get(protocol_offset)?,
            *frame.get(protocol_offset + 1)?,
        ]);
        (protocol == ETHER_TYPE_IPV4)
            .then(|| frame.get(header_bytes..))
            .flatten()
    }
}
