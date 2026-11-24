//! The server's side of the handshake.

use bonding_protocol::freewheel::{
    HELLO_MESSAGE_LEN, HELLO_PAYLOAD_LEN, PUBLIC_KEY_LEN, SessionId, WELCOME_MESSAGE_LEN,
};
use snow::{Builder, HandshakeState};

use super::handshake_error::HandshakeError;
use super::prologue::prologue;
use super::public_key::PublicKey;
use super::server_keys::ServerKeys;
use super::transport::Transport;

/// A hello the server read: who sent it and its number, until the server answers it with a
/// welcome.
pub struct Hello {
    state: HandshakeState,
    box_key: PublicKey,
    handshake_number: u64,
    ephemeral: [u8; PUBLIC_KEY_LEN],
}

impl core::fmt::Debug for Hello {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Hello")
            .field("box_key", &self.box_key)
            .field("handshake_number", &self.handshake_number)
            .finish_non_exhaustive()
    }
}

/// Reads a hello's handshake message for `session_id`: a box on the authorised list, its
/// handshake number. Costs two X25519 operations, before the box is known.
///
/// # Errors
/// A message of the wrong length, one that does not authenticate (another server key, another
/// session id, altered), or a box not on the list.
pub fn read_hello(
    keys: &ServerKeys,
    session_id: SessionId,
    message: &[u8],
) -> Result<Hello, HandshakeError> {
    if message.len() != HELLO_MESSAGE_LEN {
        return Err(HandshakeError::WrongLength { len: message.len() });
    }
    let prologue = prologue(session_id);
    let mut state = Builder::new(keys.cipher.noise_params())
        .local_private_key(keys.private.bytes())
        .and_then(|builder| builder.prologue(&prologue))
        .and_then(Builder::build_responder)
        .map_err(|_| HandshakeError::Failed)?;
    let mut payload = [0u8; HELLO_MESSAGE_LEN];
    let len = state
        .read_message(message, &mut payload)
        .map_err(|_| HandshakeError::Failed)?;
    let number = payload
        .get(..len)
        .and_then(|payload| <[u8; HELLO_PAYLOAD_LEN]>::try_from(payload).ok())
        .ok_or(HandshakeError::WrongLength { len })?;
    let box_key = state
        .get_remote_static()
        .and_then(|key| <[u8; PUBLIC_KEY_LEN]>::try_from(key).ok())
        .map(PublicKey::from_bytes)
        .ok_or(HandshakeError::Failed)?;
    if !keys.authorized.contains(&box_key) {
        return Err(HandshakeError::UnknownBox(box_key));
    }
    let ephemeral = message
        .get(..PUBLIC_KEY_LEN)
        .and_then(|key| <[u8; PUBLIC_KEY_LEN]>::try_from(key).ok())
        .ok_or(HandshakeError::Failed)?;
    Ok(Hello {
        state,
        box_key,
        handshake_number: u64::from_be_bytes(number),
        ephemeral,
    })
}

impl Hello {
    /// The box that sent it.
    #[must_use]
    pub const fn box_key(&self) -> PublicKey {
        self.box_key
    }

    /// Its number in the session.
    #[must_use]
    pub const fn handshake_number(&self) -> u64 {
        self.handshake_number
    }

    /// The box's ephemeral key: the same in every copy of one hello (every link, every retry),
    /// so a copy is told from a new hello.
    #[must_use]
    pub const fn ephemeral(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.ephemeral
    }

    /// Answers it: writes the welcome's handshake message into `out` and returns the session's
    /// transport.
    ///
    /// # Errors
    /// `snow` refused (no randomness for the ephemeral key).
    pub fn welcome(
        mut self,
        keys: &ServerKeys,
        out: &mut [u8; WELCOME_MESSAGE_LEN],
    ) -> Result<Transport, HandshakeError> {
        let len = self
            .state
            .write_message(&[], out)
            .map_err(|_| HandshakeError::Failed)?;
        if len != WELCOME_MESSAGE_LEN || !self.state.is_handshake_finished() {
            return Err(HandshakeError::WrongLength { len });
        }
        let (box_to_server, server_to_box) = self.state.dangerously_get_raw_split();
        Ok(Transport::new(
            keys.cipher,
            &server_to_box,
            &box_to_server,
            keys.replay_window_packets,
        ))
    }
}
