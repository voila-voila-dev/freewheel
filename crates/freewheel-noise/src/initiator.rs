//! The box's side of the handshake.

use bonding_protocol::freewheel::{HELLO_MESSAGE_LEN, SessionId, WELCOME_MESSAGE_LEN};
use snow::{Builder, HandshakeState};

use super::box_keys::BoxKeys;
use super::cipher::Cipher;
use super::handshake_error::HandshakeError;
use super::prologue::prologue;
use super::transport::Transport;

/// A handshake the box started: the hello to send (the same bytes on every link and every
/// retry, so the server can answer each with the same welcome), until a welcome opens the
/// session.
pub struct Initiator {
    state: HandshakeState,
    cipher: Cipher,
    replay_window_packets: usize,
    hello: [u8; HELLO_MESSAGE_LEN],
    handshake_number: u64,
}

impl core::fmt::Debug for Initiator {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Initiator")
            .field("handshake_number", &self.handshake_number)
            .finish_non_exhaustive()
    }
}

impl Initiator {
    /// Starts handshake number `handshake_number` of `session_id` (from 1, one more for each
    /// new handshake of the session: the server takes a hello only if its number is above the
    /// last it took, so a replayed one is dropped).
    ///
    /// # Errors
    /// `snow` refused the keys, or no randomness for the ephemeral key.
    pub fn start(
        keys: &BoxKeys,
        session_id: SessionId,
        handshake_number: u64,
    ) -> Result<Self, HandshakeError> {
        let prologue = prologue(session_id);
        let mut state = Builder::new(keys.cipher.noise_params())
            .local_private_key(keys.private.bytes())
            .and_then(|builder| builder.remote_public_key(keys.server.bytes()))
            .and_then(|builder| builder.prologue(&prologue))
            .and_then(Builder::build_initiator)
            .map_err(|_| HandshakeError::Failed)?;
        let mut hello = [0u8; HELLO_MESSAGE_LEN];
        let len = state
            .write_message(&handshake_number.to_be_bytes(), &mut hello)
            .map_err(|_| HandshakeError::Failed)?;
        if len != HELLO_MESSAGE_LEN {
            return Err(HandshakeError::WrongLength { len });
        }
        Ok(Self {
            state,
            cipher: keys.cipher,
            replay_window_packets: keys.replay_window_packets,
            hello,
            handshake_number,
        })
    }

    /// The hello's handshake message.
    #[must_use]
    pub const fn hello_message(&self) -> &[u8; HELLO_MESSAGE_LEN] {
        &self.hello
    }

    /// Its number in the session.
    #[must_use]
    pub const fn handshake_number(&self) -> u64 {
        self.handshake_number
    }

    /// Reads a welcome's handshake message: the session's transport if it authenticates (from
    /// the server's key, for this hello). One that does not changes nothing: the handshake
    /// still waits for the right one.
    ///
    /// # Errors
    /// A message of the wrong length, or one that does not authenticate.
    pub fn read_welcome(&mut self, message: &[u8]) -> Result<Transport, HandshakeError> {
        if message.len() != WELCOME_MESSAGE_LEN {
            return Err(HandshakeError::WrongLength { len: message.len() });
        }
        let mut payload = [0u8; WELCOME_MESSAGE_LEN];
        self.state
            .read_message(message, &mut payload)
            .map_err(|_| HandshakeError::Failed)?;
        if !self.state.is_handshake_finished() {
            return Err(HandshakeError::Failed);
        }
        let (box_to_server, server_to_box) = self.state.dangerously_get_raw_split();
        Ok(Transport::new(
            self.cipher,
            &box_to_server,
            &server_to_box,
            self.replay_window_packets,
        ))
    }
}
