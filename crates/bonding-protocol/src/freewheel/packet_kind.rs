//! The second byte of every freewheel packet: what follows the header.

/// What a packet is, from its second byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketKind {
    /// Media, control or recording bytes, in one of the three traffic classes.
    Data = 0x01,
    /// What one link received, with the timing the sender estimates that link from.
    Ack = 0x02,
    /// Keeps an idle link measured and its NAT mapping open.
    Keepalive = 0x03,
    /// A link asks to join a session.
    Join = 0x04,
    /// The server accepts a link into a session.
    Accept = 0x05,
    /// A link or the whole session ends.
    Close = 0x06,
}

impl PacketKind {
    /// Maps a raw byte to a kind; `None` for bytes v0 does not define.
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0x01 => Self::Data,
            0x02 => Self::Ack,
            0x03 => Self::Keepalive,
            0x04 => Self::Join,
            0x05 => Self::Accept,
            0x06 => Self::Close,
            _ => return None,
        })
    }

    /// The raw byte on the wire.
    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }
}
