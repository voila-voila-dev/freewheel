//! A hello or a welcome: one message of the Noise handshake that opens a session.

use super::header::Header;

/// One message of the Noise `IK` handshake, as a hello or a welcome carries it: the bytes
/// `snow` writes and reads, borrowed from the datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeMessage<'a> {
    /// The session it opens, and the link it travels on.
    pub header: Header,
    /// The handshake message: [`HELLO_MESSAGE_LEN`](super::HELLO_MESSAGE_LEN) or
    /// [`WELCOME_MESSAGE_LEN`](super::WELCOME_MESSAGE_LEN) bytes.
    pub message: &'a [u8],
}
