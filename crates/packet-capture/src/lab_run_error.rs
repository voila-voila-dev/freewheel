//! Why a lab run could not be loaded.

use core::fmt;

use crate::CaptureError;

/// Why a lab run could not be loaded.
#[derive(Debug)]
pub enum LabRunError {
    /// A file of the run is missing or unreadable.
    Unreadable {
        /// The file.
        path: String,
        /// Why.
        error: std::io::Error,
    },
    /// A capture is not a pcap file we can read.
    Capture {
        /// The file.
        path: String,
        /// Why.
        error: CaptureError,
    },
}

impl fmt::Display for LabRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, error } => write!(f, "cannot read {path}: {error}"),
            Self::Capture { path, error } => write!(f, "cannot parse {path}: {error}"),
        }
    }
}

impl std::error::Error for LabRunError {}
