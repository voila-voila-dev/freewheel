//! The 16-bit type at the start of every SRTLA and SRT control packet.

/// Control packet types carried in the first 16 bits (big-endian).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PacketType {
    /// SRT handshake.
    SrtHandshake = 0x8000,
    /// SRT keepalive.
    SrtKeepalive = 0x8001,
    /// SRT acknowledgement.
    SrtAck = 0x8002,
    /// SRT negative acknowledgement.
    SrtNak = 0x8003,
    /// SRT shutdown.
    SrtShutdown = 0x8005,
    /// SRT acknowledgement of an acknowledgement.
    SrtAckAck = 0x8006,
    /// SRTLA keepalive, sent on every link.
    Keepalive = 0x9000,
    /// SRTLA acknowledgement.
    SrtlaAck = 0x9100,
    /// Registration step 1: the sender proposes a group id.
    Reg1 = 0x9200,
    /// Registration step 2: the receiver answers with the full group id.
    Reg2 = 0x9201,
    /// Registration step 3: a link is registered.
    Reg3 = 0x9202,
    /// Registration error.
    RegErr = 0x9210,
    /// Registration failed: no group.
    RegNgp = 0x9211,
    /// Registration refused.
    RegNak = 0x9212,
}

impl PacketType {
    /// Maps a raw 16-bit type to a known control packet type.
    #[must_use]
    pub const fn from_raw(raw: u16) -> Option<Self> {
        Some(match raw {
            0x8000 => Self::SrtHandshake,
            0x8001 => Self::SrtKeepalive,
            0x8002 => Self::SrtAck,
            0x8003 => Self::SrtNak,
            0x8005 => Self::SrtShutdown,
            0x8006 => Self::SrtAckAck,
            0x9000 => Self::Keepalive,
            0x9100 => Self::SrtlaAck,
            0x9200 => Self::Reg1,
            0x9201 => Self::Reg2,
            0x9202 => Self::Reg3,
            0x9210 => Self::RegErr,
            0x9211 => Self::RegNgp,
            0x9212 => Self::RegNak,
            _ => return None,
        })
    }

    /// `true` for SRTLA's own packets, which never reach SRT.
    #[must_use]
    pub const fn is_srtla(self) -> bool {
        self.raw() >= Self::Keepalive.raw()
    }

    /// The raw 16-bit value on the wire.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self as u16
    }
}
