//! Why a datagram is not a freewheel packet we can read.

use super::packet_kind::PacketKind;

/// Why a datagram is not a freewheel packet we can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Does not start with [`MAGIC`](super::MAGIC): SRT, SRTLA, or another version.
    NotFreewheel,
    /// Shorter than the header, or than its kind's fixed fields.
    TooShort {
        /// Its length, in bytes.
        len: usize,
    },
    /// Longer than [`MAX_DATAGRAM_LEN`](super::MAX_DATAGRAM_LEN).
    TooLong {
        /// Its length, in bytes.
        len: usize,
    },
    /// A kind byte v0 does not define.
    UnknownKind {
        /// The raw byte.
        raw: u8,
    },
    /// A data packet with a class byte v0 does not define.
    UnknownClass {
        /// The raw byte.
        raw: u8,
    },
    /// A close with a reason byte v0 does not define.
    UnknownCloseReason {
        /// The raw byte.
        raw: u8,
    },
    /// The class byte is not zero on a packet that is not data.
    ReservedNotZero,
    /// A fixed-length kind of the wrong length.
    WrongLength {
        /// Its kind.
        kind: PacketKind,
        /// Its length, in bytes.
        len: usize,
    },
    /// An ACK whose ranges are not whole 8-byte pairs.
    MalformedAck,
}
