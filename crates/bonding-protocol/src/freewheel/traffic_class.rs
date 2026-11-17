//! The three traffic classes sharing one pipe.

/// How a data packet must be delivered (`docs/technical/protocol/freewheel.md`, "One pipe,
/// three traffic classes").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TrafficClass {
    /// Encoded audio and video: useless once late, so dropped or recovered before its deadline.
    Live = 0x01,
    /// Telemetry, control, GPS outbox: ordered, never lost, small.
    Reliable = 0x02,
    /// Full-quality recording upload: spare capacity only, resumed across outages.
    Background = 0x03,
}

impl TrafficClass {
    /// Maps a raw byte to a class; `None` for bytes v0 does not define.
    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0x01 => Self::Live,
            0x02 => Self::Reliable,
            0x03 => Self::Background,
            _ => return None,
        })
    }

    /// The raw byte on the wire.
    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }
}
