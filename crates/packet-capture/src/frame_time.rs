//! When one video frame went by, and the text files the lab keeps them in.

/// When one video frame went by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTime {
    /// Presentation timestamp, in 90 kHz units: the same key as a `framemd5` listing.
    pub pts: i64,
    /// When its last byte went by, in microseconds since the Unix epoch.
    pub at_us: u64,
}

impl FrameTime {
    /// One line of a `<scenario>.*.times` file: `<pts> <microseconds since the Unix epoch>`.
    #[must_use]
    pub fn line(&self) -> String {
        format!("{} {}", self.pts, self.at_us)
    }

    /// Parses a `.times` file; malformed lines are skipped.
    #[must_use]
    pub fn parse_all(text: &str) -> Vec<Self> {
        text.lines()
            .filter_map(|line| {
                let (pts, at_us) = line.split_once(' ')?;
                Some(Self {
                    pts: pts.trim().parse().ok()?,
                    at_us: at_us.trim().parse().ok()?,
                })
            })
            .collect()
    }
}
