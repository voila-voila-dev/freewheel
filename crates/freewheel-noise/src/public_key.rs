//! A Noise static public key: a box's, or the server's.

use core::fmt;
use std::path::Path;

use bonding_protocol::freewheel::PUBLIC_KEY_LEN;

use super::hex;
use super::key_error::KeyError;

/// An X25519 public key. As text (files, logs), 64 hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey([u8; PUBLIC_KEY_LEN]);

impl PublicKey {
    /// The key made of `bytes`.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; PUBLIC_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Its bytes.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; PUBLIC_KEY_LEN] {
        &self.0
    }

    /// The key written as 64 hexadecimal digits (surrounding spaces ignored); `None` for
    /// anything else.
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        hex::decode(text.trim()).map(Self)
    }

    /// Reads a public key file: the key on one line, `#` comments and empty lines allowed
    /// (what `freewheel-keygen` writes next to the private key, as `<path>.pub`).
    ///
    /// # Errors
    /// The file cannot be read, holds no key, or a line is neither.
    pub fn read_file(path: &Path) -> Result<Self, KeyError> {
        super::private_key::read_single_key(path).map(Self)
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(&self.0))
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "PublicKey({self})")
    }
}
