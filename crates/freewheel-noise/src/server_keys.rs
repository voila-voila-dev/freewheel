//! What the server needs to accept sessions.

use super::authorized_keys::AuthorizedKeys;
use super::cipher::Cipher;
use super::private_key::PrivateKey;
use super::replay_window::DEFAULT_REPLAY_WINDOW_PACKETS;

/// The server's side of the encryption: its static key, the boxes it accepts, the cipher, and
/// the replay window over what each box sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerKeys {
    /// The server's static private key (`FREEWHEEL_PRIVATE_KEY`).
    pub private: PrivateKey,
    /// The boxes' public keys (`FREEWHEEL_AUTHORIZED_KEYS`): a hello from any other is refused.
    pub authorized: AuthorizedKeys,
    /// The cipher (`FREEWHEEL_CIPHER`).
    pub cipher: Cipher,
    /// Packets the replay window over each session's packets spans
    /// (`FREEWHEEL_REPLAY_WINDOW_PACKETS`).
    pub replay_window_packets: usize,
}

impl ServerKeys {
    /// The keys with the default cipher and replay window.
    #[must_use]
    pub const fn new(private: PrivateKey, authorized: AuthorizedKeys) -> Self {
        Self {
            private,
            authorized,
            cipher: Cipher::ChaCha20Poly1305,
            replay_window_packets: DEFAULT_REPLAY_WINDOW_PACKETS,
        }
    }
}
