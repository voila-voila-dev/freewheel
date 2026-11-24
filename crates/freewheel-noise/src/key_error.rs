//! Why a key file cannot be used.

use core::fmt;
use std::path::PathBuf;

/// A key file the binaries cannot use: which, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyError {
    /// The file.
    pub path: PathBuf,
    /// What is wrong with it.
    pub problem: KeyProblem,
}

/// What is wrong with a key file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyProblem {
    /// It cannot be read (the system's reason).
    Unreadable(String),
    /// Holds no key.
    Missing,
    /// A line that is neither a key (64 hexadecimal digits), a comment (`#`) nor empty;
    /// counted from 1.
    Malformed {
        /// The line.
        line: usize,
    },
    /// A private key others may read (its Unix mode): it must be `0600` or stricter, as
    /// `freewheel-keygen` writes it.
    ReadableByOthers {
        /// Its permission bits.
        mode: u32,
    },
}

impl fmt::Display for KeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path.display();
        match &self.problem {
            KeyProblem::Unreadable(reason) => write!(formatter, "cannot read {path}: {reason}"),
            KeyProblem::Missing => write!(formatter, "{path}: no key"),
            KeyProblem::Malformed { line } => write!(
                formatter,
                "{path}:{line}: expected a key (64 hexadecimal digits), a # comment or nothing"
            ),
            KeyProblem::ReadableByOthers { mode } => write!(
                formatter,
                "{path}: a private key others may read (mode {mode:o}): chmod 600 it"
            ),
        }
    }
}

impl std::error::Error for KeyError {}
