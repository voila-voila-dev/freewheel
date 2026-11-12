//! The range of bitrates the encoder may use.

/// Bitrate bounds in bits per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Lowest bitrate the encoder may use.
    pub min_bps: u32,
    /// Highest bitrate the encoder may use.
    pub max_bps: u32,
}

impl Bounds {
    /// Keeps a bitrate inside the bounds.
    #[must_use]
    pub const fn clamp(self, bps: u32) -> u32 {
        if bps < self.min_bps {
            self.min_bps
        } else if bps > self.max_bps {
            self.max_bps
        } else {
            bps
        }
    }
}
