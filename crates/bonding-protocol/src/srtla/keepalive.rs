//! The SRTLA keepalive, echoed by the receiver.

/// An SRTLA keepalive. Upstream sends the type alone, and its receiver echoes whatever
/// follows the type, so the payload is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Keepalive<'a> {
    /// Bytes after the type; empty for upstream senders.
    pub payload: &'a [u8],
}
