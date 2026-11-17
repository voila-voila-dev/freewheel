//! Why a link or a session ends.

/// Why a link or a session ends, carried by a close packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CloseReason {
    /// The stream ended normally.
    Done = 0x00,
    /// The server refused the link (unknown session, too many links).
    Refused = 0x01,
    /// The other side heard nothing for too long.
    TimedOut = 0x02,
}

impl CloseReason {
    /// Maps a raw byte to a reason; `None` for bytes v0 does not define.
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0x00 => Self::Done,
            0x01 => Self::Refused,
            0x02 => Self::TimedOut,
            _ => return None,
        })
    }

    /// The raw byte on the wire.
    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }
}
