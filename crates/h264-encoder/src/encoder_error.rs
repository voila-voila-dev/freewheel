//! Why the encoder could not do what was asked.

use core::fmt;

/// Why the encoder could not do what was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderError {
    /// Built without libx264.
    Unavailable,
    /// Odd or zero size, or a value x264 does not take.
    InvalidSettings,
    /// x264 refused to open, encode or change its bitrate.
    Failed,
    /// The frame is not `width × height × 3 / 2` bytes.
    WrongFrameSize,
    /// The output buffer cannot hold the encoded frame.
    OutputTooSmall,
}

impl fmt::Display for EncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "built without libx264",
            Self::InvalidSettings => "invalid encoder settings",
            Self::Failed => "x264 failed",
            Self::WrongFrameSize => "frame of the wrong size",
            Self::OutputTooSmall => "output buffer too small",
        })
    }
}

impl std::error::Error for EncoderError {}
