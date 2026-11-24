//! A Noise static private key: the box's, or the server's.

use core::fmt;
use std::path::Path;

use bonding_protocol::freewheel::PUBLIC_KEY_LEN;

use super::hex;
use super::key_error::{KeyError, KeyProblem};

/// Permission bits a private key file must not have: anything for the group or others.
const OTHERS_MODE_BITS: u32 = 0o077;

/// An X25519 private key. Never printed: its [`Debug`] says only that it is one.
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateKey([u8; PUBLIC_KEY_LEN]);

impl PrivateKey {
    /// The key made of `bytes`.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; PUBLIC_KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Its bytes, for `snow`.
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

    /// The key as 64 hexadecimal digits, for its file.
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    /// Reads a private key file, as `freewheel-keygen` writes it: the key on one line, `#`
    /// comments and empty lines allowed, and no permission for the group or others.
    ///
    /// # Errors
    /// The file cannot be read, others may read it, it holds no key, or a line is neither.
    pub fn read_file(path: &Path) -> Result<Self, KeyError> {
        let error = |problem| KeyError {
            path: path.to_path_buf(),
            problem,
        };
        let mode = file_mode(path).map_err(|reason| error(KeyProblem::Unreadable(reason)))?;
        if mode & OTHERS_MODE_BITS != 0 {
            return Err(error(KeyProblem::ReadableByOthers { mode: mode & 0o777 }));
        }
        read_single_key(path).map(Self)
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PrivateKey(..)")
    }
}

#[cfg(unix)]
fn file_mode(path: &Path) -> Result<u32, String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode())
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn file_mode(_: &Path) -> Result<u32, String> {
    Ok(0)
}

/// The one key a key file holds: its only line that is not empty nor a `#` comment.
pub(crate) fn read_single_key(path: &Path) -> Result<[u8; PUBLIC_KEY_LEN], KeyError> {
    let error = |problem| KeyError {
        path: path.to_path_buf(),
        problem,
    };
    let text = std::fs::read_to_string(path)
        .map_err(|reason| error(KeyProblem::Unreadable(reason.to_string())))?;
    let mut key = None;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match hex::decode(line) {
            Some(bytes) if key.is_none() => key = Some(bytes),
            _ => return Err(error(KeyProblem::Malformed { line: index + 1 })),
        }
    }
    key.ok_or_else(|| error(KeyProblem::Missing))
}
