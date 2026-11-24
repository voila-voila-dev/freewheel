//! What the encrypted receiver dropped, and why.

/// Packets the encrypted receiver dropped unanswered, by reason, since it started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RejectedPackets {
    /// Sealed packets for a session the receiver has no keys for (unknown, or gone).
    pub unknown_session: u64,
    /// Sealed packets whose nonce was opened already, or is below the replay window.
    pub replayed: u64,
    /// Sealed packets that did not authenticate.
    pub forged: u64,
    /// Hellos refused: forged, from a box not on the list, for a session another box opened,
    /// or replayed (a handshake number not above the session's).
    pub hellos: u64,
}
