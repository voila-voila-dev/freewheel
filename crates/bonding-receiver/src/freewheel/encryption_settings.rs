//! How the receiver encrypts (M1b step 7): its key, the boxes it accepts, the cipher and the
//! replay window, read from the environment by `freewheel-receiver`.

use std::path::PathBuf;

use freewheel_noise::{
    AuthorizedKeys, Cipher, DEFAULT_REPLAY_WINDOW_PACKETS, KeyError, MAX_REPLAY_WINDOW_PACKETS,
    PrivateKey, ServerKeys,
};

use super::setting_error::SettingError;

/// `1`: wire format v0 in clear, no key needed; the boxes must be in clear too. Unset, empty
/// or `0`: encrypted (the default).
pub const PLAINTEXT_VARIABLE: &str = "FREEWHEEL_PLAINTEXT";
/// [`EncryptionSettings::private_key_path`].
pub const PRIVATE_KEY_VARIABLE: &str = "FREEWHEEL_PRIVATE_KEY";
/// [`EncryptionSettings::authorized_keys_path`].
pub const AUTHORIZED_KEYS_VARIABLE: &str = "FREEWHEEL_AUTHORIZED_KEYS";
/// [`EncryptionSettings::cipher`].
pub const CIPHER_VARIABLE: &str = "FREEWHEEL_CIPHER";
/// [`EncryptionSettings::replay_window_packets`].
pub const REPLAY_WINDOW_PACKETS_VARIABLE: &str = "FREEWHEEL_REPLAY_WINDOW_PACKETS";

/// Where the server's keys are, which cipher it seals with, how far back it takes packets.
/// Each field reads from one environment variable (the `*_VARIABLE` constants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionSettings {
    /// The server's static private key, a file `freewheel-keygen` wrote (mode 0600)
    /// ([`PRIVATE_KEY_VARIABLE`], required).
    pub private_key_path: PathBuf,
    /// The boxes it accepts: one public key per line, a name after it allowed, `#` comments
    /// ([`AUTHORIZED_KEYS_VARIABLE`], required; read once, at start).
    pub authorized_keys_path: PathBuf,
    /// The AEAD, `chacha20poly1305` or `aes256gcm`, the same as the boxes'
    /// ([`CIPHER_VARIABLE`], default `chacha20poly1305`).
    pub cipher: Cipher,
    /// Packets each session's replay window spans, from 64 to [`MAX_REPLAY_WINDOW_PACKETS`]:
    /// a packet whose nonce is further behind the latest is dropped
    /// ([`REPLAY_WINDOW_PACKETS_VARIABLE`], default [`DEFAULT_REPLAY_WINDOW_PACKETS`]).
    pub replay_window_packets: usize,
}

/// Smallest replay window: one word of bits.
const MIN_REPLAY_WINDOW_PACKETS: usize = 64;

impl EncryptionSettings {
    /// Reads the settings through `variable` (`std::env::var` in the binary): `None` when
    /// [`PLAINTEXT_VARIABLE`] is `1` (in clear), else the key paths, which must be set, and the
    /// defaults for the rest. An empty variable counts as unset.
    ///
    /// # Errors
    /// A variable holds what its setting cannot be, or a key path is missing.
    pub fn from_environment(
        variable: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, SettingError> {
        let read = |name: &str| {
            variable(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        match read(PLAINTEXT_VARIABLE).as_deref() {
            Some("1") => return Ok(None),
            None | Some("0") => {}
            Some(value) => {
                return Err(SettingError {
                    variable: PLAINTEXT_VARIABLE,
                    value: value.to_owned(),
                    expected: "1 (in clear) or 0 (encrypted, the default)".to_owned(),
                });
            }
        }
        let path = |name: &'static str| {
            read(name).map(PathBuf::from).ok_or_else(|| SettingError {
                variable: name,
                value: String::new(),
                expected: format!("a key file (or {PLAINTEXT_VARIABLE}=1 to run in clear)"),
            })
        };
        let cipher = match read(CIPHER_VARIABLE) {
            None => Cipher::default(),
            Some(name) => Cipher::from_name(&name).ok_or_else(|| SettingError {
                variable: CIPHER_VARIABLE,
                value: name,
                expected: "chacha20poly1305 or aes256gcm".to_owned(),
            })?,
        };
        let replay_window_packets = match read(REPLAY_WINDOW_PACKETS_VARIABLE) {
            None => DEFAULT_REPLAY_WINDOW_PACKETS,
            Some(value) => match value.parse::<usize>() {
                Ok(packets)
                    if (MIN_REPLAY_WINDOW_PACKETS..=MAX_REPLAY_WINDOW_PACKETS)
                        .contains(&packets) =>
                {
                    packets
                }
                _ => {
                    return Err(SettingError {
                        variable: REPLAY_WINDOW_PACKETS_VARIABLE,
                        value,
                        expected: format!(
                            "a number of packets from {MIN_REPLAY_WINDOW_PACKETS} to {MAX_REPLAY_WINDOW_PACKETS}"
                        ),
                    });
                }
            },
        };
        Ok(Some(Self {
            private_key_path: path(PRIVATE_KEY_VARIABLE)?,
            authorized_keys_path: path(AUTHORIZED_KEYS_VARIABLE)?,
            cipher,
            replay_window_packets,
        }))
    }

    /// Reads the key files.
    ///
    /// # Errors
    /// A file cannot be read or holds what is not a key, or the private key is readable by
    /// others.
    pub fn read_keys(&self) -> Result<ServerKeys, KeyError> {
        let mut keys = ServerKeys::new(
            PrivateKey::read_file(&self.private_key_path)?,
            AuthorizedKeys::read_file(&self.authorized_keys_path)?,
        );
        keys.cipher = self.cipher;
        keys.replay_window_packets = self.replay_window_packets;
        Ok(keys)
    }
}
