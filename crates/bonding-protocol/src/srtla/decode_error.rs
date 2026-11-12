//! Why a datagram is not a packet we can read.

/// Why a datagram is not a packet we can read. Upstream drops such datagrams silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Too short for any packet it could be: under 2 bytes, or under 16 bytes without being
    /// an SRTLA packet of its exact length.
    TooShort {
        /// Its length, in bytes.
        len: usize,
    },
    /// Longer than the largest datagram either side reads.
    TooLong {
        /// Its length, in bytes.
        len: usize,
    },
    /// Typed as an SRTLA ACK, but not padded with zeros or not made of whole 31-bit
    /// sequence numbers.
    MalformedSrtlaAck,
}
