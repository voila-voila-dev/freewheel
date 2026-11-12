//! Why the sender stopped.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Why [`serve`](super::serve) stopped before being asked to.
#[derive(Debug)]
pub enum ServeError {
    /// Every link failed for too long (upstream exits with status 1).
    GaveUp,
    /// The source list could not be reread on `SIGHUP` (upstream exits too).
    SourceListUnreadable {
        /// The file.
        path: PathBuf,
        /// Why.
        error: io::Error,
    },
    /// Waiting on the sockets failed, which only happens when the process is out of resources.
    Wait(io::Error),
}

impl fmt::Display for ServeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GaveUp => write!(formatter, "no link to the receiver for too long, giving up"),
            Self::SourceListUnreadable { path, error } => {
                write!(formatter, "cannot read {}: {error}", path.display())
            }
            Self::Wait(error) => write!(formatter, "waiting on the sockets failed: {error}"),
        }
    }
}

impl std::error::Error for ServeError {}
