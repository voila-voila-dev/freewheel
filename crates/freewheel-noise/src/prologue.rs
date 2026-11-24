//! What both sides bind the handshake to before it starts.

use bonding_protocol::freewheel::SessionId;

/// Names the protocol and its version, so a handshake for anything else fails.
const LABEL: &[u8; 12] = b"freewheel v1";

/// The handshake's prologue: the protocol and the session id, so a hello replayed under
/// another session id does not authenticate. The link id is left out: the same hello goes on
/// every link.
pub(crate) fn prologue(session_id: SessionId) -> [u8; 20] {
    let mut bytes = [0u8; 20];
    let (label, session) = bytes.split_at_mut(LABEL.len());
    label.copy_from_slice(LABEL);
    session.copy_from_slice(&session_id.to_be_bytes());
    bytes
}
