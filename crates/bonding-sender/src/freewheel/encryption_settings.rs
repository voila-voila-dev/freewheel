//! How the sender encrypts (M1b step 7): its key files and cipher, read from the environment by
//! `freewheel-sender`.

use std::path::PathBuf;

use freewheel_noise::{BoxKeys, Cipher, KeyError, PrivateKey, PublicKey};

use super::setting_error::SettingError;

/// `1`: wire format v0 in clear, no key needed; the server must be in clear too. Unset, empty
/// or `0`: encrypted (the default).
pub const PLAINTEXT_VARIABLE: &str = "FREEWHEEL_PLAINTEXT";
/// [`EncryptionSettings::private_key_path`].
pub const PRIVATE_KEY_VARIABLE: &str = "FREEWHEEL_PRIVATE_KEY";
/// [`EncryptionSettings::server_public_key_path`].
pub const SERVER_PUBLIC_KEY_VARIABLE: &str = "FREEWHEEL_SERVER_PUBLIC_KEY";
/// [`EncryptionSettings::cipher`].
pub const CIPHER_VARIABLE: &str = "FREEWHEEL_CIPHER";

/// Where the box's keys are and which cipher it seals with. Each field reads from one
/// environment variable (the `*_VARIABLE` constants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionSettings {
    /// The box's static private key, a file `freewheel-keygen` wrote (mode 0600)
    /// ([`PRIVATE_KEY_VARIABLE`], required).
    pub private_key_path: PathBuf,
    /// The server's static public key, a file: a server with another key cannot open the
    /// session ([`SERVER_PUBLIC_KEY_VARIABLE`], required).
    pub server_public_key_path: PathBuf,
    /// The AEAD, `chacha20poly1305` or `aes256gcm`, the same as the server's
    /// ([`CIPHER_VARIABLE`], default `chacha20poly1305`).
    pub cipher: Cipher,
}

impl EncryptionSettings {
    /// Reads the settings through `variable` (`std::env::var` in the binary): `None` when
    /// [`PLAINTEXT_VARIABLE`] is `1` (in clear), else the key paths, which must be set. An
    /// empty variable counts as unset.
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
        Ok(Some(Self {
            private_key_path: path(PRIVATE_KEY_VARIABLE)?,
            server_public_key_path: path(SERVER_PUBLIC_KEY_VARIABLE)?,
            cipher,
        }))
    }

    /// Reads the key files.
    ///
    /// # Errors
    /// A file cannot be read, holds no key, or the private key is readable by others.
    pub fn read_keys(&self) -> Result<BoxKeys, KeyError> {
        let mut keys = BoxKeys::new(
            PrivateKey::read_file(&self.private_key_path)?,
            PublicKey::read_file(&self.server_public_key_path)?,
        );
        keys.cipher = self.cipher;
        Ok(keys)
    }
}
