//! Why a handshake did not open a session.

use super::public_key::PublicKey;

/// Why a handshake did not open a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeError {
    /// The message did not authenticate (forged, altered, from another session or for another
    /// server key), or `snow` refused a step.
    Failed,
    /// The box's key is not in the server's [`AuthorizedKeys`](super::AuthorizedKeys).
    UnknownBox(PublicKey),
    /// A message of the wrong length.
    WrongLength {
        /// Its length, in bytes.
        len: usize,
    },
}
