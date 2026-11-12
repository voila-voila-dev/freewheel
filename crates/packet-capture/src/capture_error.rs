//! Why a capture could not be read.

use core::fmt;

/// Why a capture could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    /// The file does not start with the classic little-endian, microsecond pcap magic.
    UnsupportedFormat,
    /// The link layer is neither Ethernet nor Linux cooked capture.
    UnsupportedLinkType {
        /// Link type found in the global header.
        link_type: u32,
    },
    /// The file ends in the middle of a header or a frame.
    Truncated {
        /// Byte offset where reading stopped.
        offset: usize,
    },
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat => write!(f, "not a little-endian microsecond pcap file"),
            Self::UnsupportedLinkType { link_type } => {
                write!(f, "link type {link_type} is not supported")
            }
            Self::Truncated { offset } => write!(f, "file truncated at byte {offset}"),
        }
    }
}

impl std::error::Error for CaptureError {}
