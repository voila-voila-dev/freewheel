//! The session and link every packet belongs to.

/// Identifies a stream, whatever addresses its links use: an IP change is a non-event.
pub type SessionId = u64;

/// Identifies one link (one modem, one Wi-Fi) within a session.
pub type LinkId = u8;

/// The part of the header that names who a packet belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Header {
    /// The session.
    pub session_id: SessionId,
    /// The link the packet is about: the one it travels on, or the one an ACK reports.
    pub link_id: LinkId,
}
