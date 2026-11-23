//! Why the payload of a background data packet is not one we can read.

/// Why the payload of a background data packet is not one we can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundDecodeError {
    /// No kind byte.
    Empty,
    /// Longer than [`MAX_DATA_PAYLOAD_LEN`](super::MAX_DATA_PAYLOAD_LEN).
    TooLong {
        /// Its length, in bytes.
        len: usize,
    },
    /// A kind byte v0 does not define.
    UnknownKind {
        /// The raw byte.
        raw: u8,
    },
    /// Padding with a byte other than zero.
    PaddingNotZero,
    /// A recording chunk shorter than its header.
    ChunkTooShort {
        /// Its length, in bytes.
        len: usize,
    },
    /// A recording chunk without a byte of the recording.
    EmptyChunk,
}
