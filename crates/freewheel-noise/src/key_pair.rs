//! A new static key pair, for a box or for the server.

use snow::Builder;

use super::cipher::Cipher;
use super::handshake_error::HandshakeError;
use super::private_key::PrivateKey;
use super::public_key::PublicKey;

/// A static key pair: the private key stays on its machine, the public key goes to the other
/// side (the server's to every box, a box's to the server's [`AuthorizedKeys`](super::AuthorizedKeys)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPair {
    /// The private key.
    pub private: PrivateKey,
    /// The public key.
    pub public: PublicKey,
}

impl KeyPair {
    /// A new key pair, from the system's randomness (through `snow`). The same pair serves
    /// both ciphers.
    ///
    /// # Errors
    /// No randomness.
    pub fn generate() -> Result<Self, HandshakeError> {
        let keypair = Builder::new(Cipher::ChaCha20Poly1305.noise_params())
            .generate_keypair()
            .map_err(|_| HandshakeError::Failed)?;
        let (Ok(private), Ok(public)) = (
            <[u8; 32]>::try_from(keypair.private.as_slice()),
            <[u8; 32]>::try_from(keypair.public.as_slice()),
        ) else {
            return Err(HandshakeError::Failed);
        };
        Ok(Self {
            private: PrivateKey::from_bytes(private),
            public: PublicKey::from_bytes(public),
        })
    }
}
