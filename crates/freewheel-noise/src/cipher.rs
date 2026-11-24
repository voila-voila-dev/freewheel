//! Which AEAD seals the packets.

use snow::params::{
    BaseChoice, CipherChoice, DHChoice, HandshakeChoice, HandshakeModifierList, HandshakePattern,
    HashChoice, NoiseParams,
};

/// The AEAD of a session: the handshake's and the transport's. Both sides must use the same
/// (`FREEWHEEL_CIPHER`); ChaCha20-Poly1305 by default until the ROCK 5T benchmark of M3
/// decides (`decisions.md`, 2026-11-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cipher {
    /// ChaCha20-Poly1305: fast without AES instructions.
    #[default]
    ChaCha20Poly1305,
    /// AES-256-GCM: fast with them (the ROCK 5T's Cortex-A76 has them).
    Aes256Gcm,
}

/// The Noise protocol names, as the handshake hashes them.
const CHACHA_PROTOCOL: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";
const AES_PROTOCOL: &str = "Noise_IK_25519_AESGCM_BLAKE2s";

impl Cipher {
    /// The cipher a setting names (`chacha20poly1305`, `aes256gcm`, any case); `None` for
    /// anything else.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "chacha20poly1305" => Some(Self::ChaCha20Poly1305),
            "aes256gcm" => Some(Self::Aes256Gcm),
            _ => None,
        }
    }

    /// The name a setting gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ChaCha20Poly1305 => "chacha20poly1305",
            Self::Aes256Gcm => "aes256gcm",
        }
    }

    /// The Noise protocol of the handshake with this cipher: `IK`, X25519, the cipher,
    /// BLAKE2s.
    pub(crate) fn noise_params(self) -> NoiseParams {
        let (name, cipher) = match self {
            Self::ChaCha20Poly1305 => (CHACHA_PROTOCOL, CipherChoice::ChaChaPoly),
            Self::Aes256Gcm => (AES_PROTOCOL, CipherChoice::AESGCM),
        };
        NoiseParams::new(
            name.to_owned(),
            BaseChoice::Noise,
            HandshakeChoice {
                pattern: HandshakePattern::IK,
                modifiers: HandshakeModifierList { list: Vec::new() },
            },
            DHChoice::Curve25519,
            cipher,
            HashChoice::Blake2s,
        )
    }
}
