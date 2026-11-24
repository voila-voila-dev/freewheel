//! Why a packet was not sealed.

use bonding_protocol::freewheel::EncodeError;

/// Why a packet was not sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// It cannot be framed: not a v0 packet, too long once sealed, or `out` too small.
    Frame(EncodeError),
    /// The AEAD refused (a buffer beyond its limit: never with one datagram).
    Aead,
}
