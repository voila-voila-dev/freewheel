//! The boxes the server accepts.

use std::path::Path;

use super::key_error::{KeyError, KeyProblem};
use super::public_key::PublicKey;

/// The public keys of the boxes the server accepts, from a file: one key per line (64
/// hexadecimal digits), anything after it on the line a name for the logs, `#` comments and
/// empty lines allowed. Resetting a box's keys: a new pair (`freewheel-keygen`), its new public
/// key in, its old one out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuthorizedKeys {
    keys: Vec<PublicKey>,
}

impl AuthorizedKeys {
    /// A list of exactly `keys`.
    #[must_use]
    pub const fn new(keys: Vec<PublicKey>) -> Self {
        Self { keys }
    }

    /// Reads the list from `text`; a line that is not a key, a comment or empty is refused,
    /// with its number (from 1).
    ///
    /// # Errors
    /// The number of the first malformed line.
    pub fn parse(text: &str) -> Result<Self, usize> {
        let mut keys = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let key = line.split_whitespace().next().unwrap_or_default();
            keys.push(PublicKey::from_hex(key).ok_or(index + 1)?);
        }
        Ok(Self { keys })
    }

    /// Reads the list from a file. An empty list is allowed (no box gets in).
    ///
    /// # Errors
    /// The file cannot be read, or a line is malformed.
    pub fn read_file(path: &Path) -> Result<Self, KeyError> {
        let error = |problem| KeyError {
            path: path.to_path_buf(),
            problem,
        };
        let text = std::fs::read_to_string(path)
            .map_err(|reason| error(KeyProblem::Unreadable(reason.to_string())))?;
        Self::parse(&text).map_err(|line| error(KeyProblem::Malformed { line }))
    }

    /// `true` when `key` is on the list.
    #[must_use]
    pub fn contains(&self, key: &PublicKey) -> bool {
        self.keys.contains(key)
    }

    /// How many keys it holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.keys.len()
    }

    /// `true` when no box gets in.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}
