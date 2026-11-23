//! The server's copy of the recording against the recording itself, byte for byte.

/// How much of a recording the server's copy holds, byte for byte (M1b step 6). The lab writes
/// it as `<scenario>.recording`, one line: `recording_bytes=<n> copy_bytes=<n>
/// identical_bytes=<n>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RecordingComparison {
    /// Length of the recording.
    pub recording_bytes: u64,
    /// Length of the copy (holes the sender never filled read as zeros).
    pub copy_bytes: u64,
    /// Bytes of the recording found at the same offset in the copy. A hole matches where the
    /// recording holds a zero (1 byte in 256 of random bytes): the share is exact only at 100 %.
    pub identical_bytes: u64,
}

const RECORDING_KEY: &str = "recording_bytes";
const COPY_KEY: &str = "copy_bytes";
const IDENTICAL_KEY: &str = "identical_bytes";
const PERCENT: f64 = 100.0;
/// The most a copy that is not whole shows, so a table rounding to a tenth never says 100.
const ALMOST_ALL_PERCENT: f64 = 99.9;

impl RecordingComparison {
    /// Compares `copy` with `recording`, byte for byte.
    #[must_use]
    pub fn compare(recording: &[u8], copy: &[u8]) -> Self {
        let identical = recording
            .iter()
            .zip(copy)
            .filter(|(original, copied)| original == copied)
            .count();
        Self {
            recording_bytes: recording.len() as u64,
            copy_bytes: copy.len() as u64,
            identical_bytes: identical as u64,
        }
    }

    /// The copy is the recording, exactly.
    #[must_use]
    pub const fn whole(&self) -> bool {
        self.recording_bytes > 0
            && self.identical_bytes == self.recording_bytes
            && self.copy_bytes == self.recording_bytes
    }

    /// The share of the recording's bytes the copy holds identical, in per cent; 100 only when
    /// [`whole`](Self::whole), never by rounding.
    #[must_use]
    pub fn delivered_percent(&self) -> Option<f64> {
        if self.recording_bytes == 0 {
            return None;
        }
        if self.whole() {
            return Some(PERCENT);
        }
        let share = to_f64(self.identical_bytes) * PERCENT / to_f64(self.recording_bytes);
        Some(share.min(ALMOST_ALL_PERCENT))
    }

    /// The one-line summary the lab keeps.
    #[must_use]
    pub fn to_line(&self) -> String {
        format!(
            "{RECORDING_KEY}={} {COPY_KEY}={} {IDENTICAL_KEY}={}",
            self.recording_bytes, self.copy_bytes, self.identical_bytes
        )
    }

    /// Reads [`to_line`](Self::to_line)'s summary; `None` if a field is missing.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let field = |key: &str| {
            text.split_whitespace()
                .find_map(|word| word.strip_prefix(key)?.strip_prefix('=')?.parse().ok())
        };
        Some(Self {
            recording_bytes: field(RECORDING_KEY)?,
            copy_bytes: field(COPY_KEY)?,
            identical_bytes: field(IDENTICAL_KEY)?,
        })
    }
}

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: u64) -> f64 {
    value as f64
}
