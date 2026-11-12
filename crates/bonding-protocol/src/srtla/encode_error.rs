//! Why a packet could not be written.

/// Why a packet could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    /// The output buffer is shorter than the packet.
    BufferTooSmall {
        /// Bytes the packet needs.
        needed: usize,
    },
    /// The packet would be longer than the largest datagram either side reads.
    TooLong {
        /// Its length, in bytes.
        len: usize,
    },
}
