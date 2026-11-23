//! How the receiver keeps the recordings the senders upload (M1b step 6, the background
//! class), read from the environment by `freewheel-receiver`.

use std::path::PathBuf;

use bonding_protocol::freewheel::MAX_ACK_RANGES;

use super::setting_error::SettingError;

/// The directory the copies are written to, one file per session: switches the recording on.
/// Unset or empty: background data is counted, acknowledged and dropped, exactly as without the
/// background class.
pub const DIRECTORY_VARIABLE: &str = "FREEWHEEL_RECORDING_DIR";
/// [`RecordingSettings::writer_queue_chunks`].
pub const WRITER_QUEUE_CHUNKS_VARIABLE: &str = "FREEWHEEL_RECORDING_QUEUE_CHUNKS";
/// [`RecordingSettings::acknowledged_ranges`].
pub const ACKNOWLEDGED_RANGES_VARIABLE: &str = "FREEWHEEL_RECORDING_ACK_RANGES";

/// Default [`RecordingSettings::writer_queue_chunks`]: 6 MB of full chunks, seconds of a disk
/// slower than the links.
pub const DEFAULT_WRITER_QUEUE_CHUNKS: usize = 4_096;
/// Default [`RecordingSettings::acknowledged_ranges`]: 136 bytes of ranges per ACK; with an ACK
/// every 4 data packets per link, each chunk is acknowledged several times, so a lost ACK
/// costs no resend.
pub const DEFAULT_ACKNOWLEDGED_RANGES: usize = 16;
/// Most [`RecordingSettings::acknowledged_ranges`]: what the receiver keeps per session.
pub const MAX_ACKNOWLEDGED_RANGES: usize = 32;

// The ranges kept must fit one ACK.
const _: () = assert!(MAX_ACKNOWLEDGED_RANGES <= MAX_ACK_RANGES);

/// Where and how the recordings are kept. Each field reads from one environment variable (the
/// `*_VARIABLE` constants), and has a default (the `DEFAULT_*` constants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingSettings {
    /// Where each session's copy is written ([`DIRECTORY_VARIABLE`]).
    pub directory: PathBuf,
    /// Chunks waiting for the disk at most; a chunk beyond is dropped unacknowledged, and the
    /// sender sends it again ([`WRITER_QUEUE_CHUNKS_VARIABLE`], default
    /// [`DEFAULT_WRITER_QUEUE_CHUNKS`]).
    pub writer_queue_chunks: usize,
    /// Ranges of recording sequences each ACK carries, the latest received in the session,
    /// from 1 to [`MAX_ACKNOWLEDGED_RANGES`] ([`ACKNOWLEDGED_RANGES_VARIABLE`], default
    /// [`DEFAULT_ACKNOWLEDGED_RANGES`]).
    pub acknowledged_ranges: usize,
}

impl RecordingSettings {
    /// Settings for `directory` with every default.
    #[must_use]
    pub const fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            writer_queue_chunks: DEFAULT_WRITER_QUEUE_CHUNKS,
            acknowledged_ranges: DEFAULT_ACKNOWLEDGED_RANGES,
        }
    }

    /// Reads the settings from environment variables, through `variable` (`std::env::var` in
    /// the binary): `None` when [`DIRECTORY_VARIABLE`] is unset or empty (no recording), else
    /// the defaults with what the other variables set. An empty variable counts as unset.
    ///
    /// # Errors
    /// A variable holds what its setting cannot be.
    pub fn from_environment(
        variable: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, SettingError> {
        let read = |name: &str| {
            variable(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let Some(directory) = read(DIRECTORY_VARIABLE) else {
            return Ok(None);
        };
        let defaults = Self::new(PathBuf::from(directory));
        let number = |name: &'static str, low: usize, high: usize, unit: &str| {
            read(name)
                .map(|value| match value.parse::<usize>() {
                    Ok(number) if (low..=high).contains(&number) => Ok(number),
                    _ => Err(SettingError {
                        variable: name,
                        value,
                        expected: format!("{unit} from {low} to {high}"),
                    }),
                })
                .transpose()
        };
        Ok(Some(Self {
            writer_queue_chunks: number(
                WRITER_QUEUE_CHUNKS_VARIABLE,
                1,
                usize::try_from(u32::MAX).unwrap_or(usize::MAX),
                "a number of chunks",
            )?
            .unwrap_or(defaults.writer_queue_chunks),
            acknowledged_ranges: number(
                ACKNOWLEDGED_RANGES_VARIABLE,
                1,
                MAX_ACKNOWLEDGED_RANGES,
                "a number of ranges",
            )?
            .unwrap_or(defaults.acknowledged_ranges),
            ..defaults
        }))
    }
}
