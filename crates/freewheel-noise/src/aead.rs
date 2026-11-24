//! One direction's AEAD: a key, sealing and opening in place.

use aes_gcm::Aes256Gcm;
use bonding_protocol::freewheel::TAG_LEN;
use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::aead::{AeadInPlace, KeyInit};

use super::cipher::Cipher;

/// Length of an AEAD key, in bytes (both ciphers).
pub(crate) const KEY_LEN: usize = 32;
/// Length of an AEAD nonce, in bytes (both ciphers): 32 zero bits, then our 64-bit counter.
const AEAD_NONCE_LEN: usize = 12;

/// One direction's key, as Noise's `CipherState` uses it, with associated data: the nonce is
/// 32 zero bits then the counter, little-endian for ChaCha20-Poly1305, big-endian for
/// AES-256-GCM (the Noise specification, section 12).
pub(crate) enum Aead {
    ChaCha(ChaCha20Poly1305),
    Aes(Box<Aes256Gcm>),
}

impl Aead {
    /// The AEAD of `cipher` with `key`.
    pub(crate) fn new(cipher: Cipher, key: &[u8; KEY_LEN]) -> Self {
        match cipher {
            Cipher::ChaCha20Poly1305 => Self::ChaCha(ChaCha20Poly1305::new(key.into())),
            Cipher::Aes256Gcm => Self::Aes(Box::new(Aes256Gcm::new(key.into()))),
        }
    }

    /// Encrypts `buffer` in place, authenticating `associated_data` too; returns the tag.
    pub(crate) fn seal(
        &self,
        nonce: u64,
        associated_data: &[u8],
        buffer: &mut [u8],
    ) -> Option<[u8; TAG_LEN]> {
        let tag = match self {
            Self::ChaCha(aead) => {
                aead.encrypt_in_place_detached(&chacha_nonce(nonce).into(), associated_data, buffer)
            }
            Self::Aes(aead) => {
                aead.encrypt_in_place_detached(&aes_nonce(nonce).into(), associated_data, buffer)
            }
        }
        .ok()?;
        Some(tag.into())
    }

    /// Checks `tag` over `associated_data` and `buffer`, then decrypts `buffer` in place;
    /// `false` (and `buffer` unspecified) when it does not authenticate.
    pub(crate) fn open(
        &self,
        nonce: u64,
        associated_data: &[u8],
        buffer: &mut [u8],
        tag: &[u8; TAG_LEN],
    ) -> bool {
        match self {
            Self::ChaCha(aead) => aead
                .decrypt_in_place_detached(
                    &chacha_nonce(nonce).into(),
                    associated_data,
                    buffer,
                    tag.into(),
                )
                .is_ok(),
            Self::Aes(aead) => aead
                .decrypt_in_place_detached(
                    &aes_nonce(nonce).into(),
                    associated_data,
                    buffer,
                    tag.into(),
                )
                .is_ok(),
        }
    }
}

fn chacha_nonce(counter: u64) -> [u8; AEAD_NONCE_LEN] {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    if let Some(low) = nonce.get_mut(4..) {
        low.copy_from_slice(&counter.to_le_bytes());
    }
    nonce
}

fn aes_nonce(counter: u64) -> [u8; AEAD_NONCE_LEN] {
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    if let Some(low) = nonce.get_mut(4..) {
        low.copy_from_slice(&counter.to_be_bytes());
    }
    nonce
}
