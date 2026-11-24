//! Why a datagram is not a freewheel v1 packet we can read.

/// Why a datagram is not a freewheel v1 packet we can read, before any decryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeDecodeError {
    /// Does not start with [`SEALED_MAGIC`](super::SEALED_MAGIC): v0, SRT or SRTLA.
    NotSealed,
    /// Shorter than its kind needs.
    TooShort {
        /// Its length, in bytes.
        len: usize,
    },
    /// Longer than [`MAX_DATAGRAM_LEN`](super::MAX_DATAGRAM_LEN).
    TooLong {
        /// Its length, in bytes.
        len: usize,
    },
    /// A kind byte v1 does not define.
    UnknownKind {
        /// The raw byte.
        raw: u8,
    },
    /// A sealed data packet with a class byte v1 does not define.
    UnknownClass {
        /// The raw byte.
        raw: u8,
    },
    /// The class byte is not zero on a packet that is not data.
    ReservedNotZero,
    /// A kind of fixed length with another length, or an ACK whose ranges are not whole
    /// pairs.
    WrongLength {
        /// The raw kind byte.
        kind: u8,
        /// Its length, in bytes.
        len: usize,
    },
}
