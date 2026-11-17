//! Why a freewheel packet could not be written.

/// Why a freewheel packet could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The output buffer is shorter than the packet.
    BufferTooSmall {
        /// Bytes the packet needs.
        needed: usize,
    },
    /// The packet would be longer than [`MAX_DATAGRAM_LEN`](super::MAX_DATAGRAM_LEN).
    TooLong {
        /// Its length, in bytes.
        len: usize,
    },
}
