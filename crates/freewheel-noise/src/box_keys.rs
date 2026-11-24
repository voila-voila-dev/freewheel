//! What a box needs to open sessions.

use super::cipher::Cipher;
use super::private_key::PrivateKey;
use super::public_key::PublicKey;
use super::replay_window::DEFAULT_REPLAY_WINDOW_PACKETS;

/// A box's side of the encryption: its own static key, the server's public key it trusts, the
/// cipher, and the replay window over what the server sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxKeys {
    /// The box's static private key (`FREEWHEEL_PRIVATE_KEY`).
    pub private: PrivateKey,
    /// The server's static public key (`FREEWHEEL_SERVER_PUBLIC_KEY`): a welcome from any
    /// other key does not authenticate.
    pub server: PublicKey,
    /// The cipher (`FREEWHEEL_CIPHER`).
    pub cipher: Cipher,
    /// Packets the replay window over the server's packets spans.
    pub replay_window_packets: usize,
}

impl BoxKeys {
    /// The keys with the default cipher and replay window.
    #[must_use]
    pub const fn new(private: PrivateKey, server: PublicKey) -> Self {
        Self {
            private,
            server,
            cipher: Cipher::ChaCha20Poly1305,
            replay_window_packets: DEFAULT_REPLAY_WINDOW_PACKETS,
        }
    }
}
