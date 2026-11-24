//! How the sender uploads the full-quality recording (M1b step 6, the background class): every
//! number of the upload, read from the environment by `freewheel-sender`.

use std::path::PathBuf;

use bonding_protocol::freewheel::MAX_RECORDING_CHUNK_LEN;

use super::setting_error::SettingError;

/// The recording to upload: switches the upload on. Unset or empty: no upload, and the sender
/// behaves exactly as without the background class (its capacity probing pads instead).
pub const RECORDING_VARIABLE: &str = "FREEWHEEL_RECORDING";
/// [`RecordingSettings::chunk_bytes`].
pub const CHUNK_BYTES_VARIABLE: &str = "FREEWHEEL_RECORDING_CHUNK_BYTES";
/// [`RecordingSettings::share_percent`].
pub const SHARE_PERCENT_VARIABLE: &str = "FREEWHEEL_RECORDING_SHARE_PERCENT";
/// [`RecordingSettings::live_ratio_percent`].
pub const LIVE_RATIO_PERCENT_VARIABLE: &str = "FREEWHEEL_RECORDING_LIVE_RATIO_PERCENT";
/// [`RecordingSettings::queue_target_us`], in milliseconds.
pub const QUEUE_TARGET_MS_VARIABLE: &str = "FREEWHEEL_RECORDING_QUEUE_TARGET_MS";
/// [`RecordingSettings::base_rtt_window_us`], in seconds.
pub const BASE_RTT_WINDOW_S_VARIABLE: &str = "FREEWHEEL_RECORDING_BASE_RTT_WINDOW_S";
/// [`RecordingSettings::window_gain_chunks`].
pub const WINDOW_GAIN_CHUNKS_VARIABLE: &str = "FREEWHEEL_RECORDING_WINDOW_GAIN_CHUNKS";
/// [`RecordingSettings::idle_live_bps`].
pub const IDLE_LIVE_BPS_VARIABLE: &str = "FREEWHEEL_RECORDING_IDLE_LIVE_BPS";
/// [`RecordingSettings::loss_timeout_us`], in milliseconds.
pub const LOSS_TIMEOUT_MS_VARIABLE: &str = "FREEWHEEL_RECORDING_LOSS_TIMEOUT_MS";
/// [`RecordingSettings::end_idle_us`], in milliseconds.
pub const END_IDLE_MS_VARIABLE: &str = "FREEWHEEL_RECORDING_END_IDLE_MS";
/// [`RecordingSettings::drain_us`], in seconds.
pub const DRAIN_S_VARIABLE: &str = "FREEWHEEL_RECORDING_DRAIN_S";
/// [`RecordingSettings::window_bytes`].
pub const WINDOW_BYTES_VARIABLE: &str = "FREEWHEEL_RECORDING_WINDOW_BYTES";
/// [`RecordingSettings::in_flight_chunks`].
pub const IN_FLIGHT_CHUNKS_VARIABLE: &str = "FREEWHEEL_RECORDING_IN_FLIGHT_CHUNKS";

/// Default [`RecordingSettings::chunk_bytes`]: the most one datagram carries.
pub const DEFAULT_CHUNK_BYTES: usize = MAX_RECORDING_CHUNK_LEN;
/// Default [`RecordingSettings::share_percent`]: all of it. The congestion window is twice the
/// bandwidth-delay product, so this leaves the upload room to find more than the links are
/// estimated to carry; what keeps the live stream from waiting is the queue target.
pub const DEFAULT_SHARE_PERCENT: u64 = 100;
/// Default [`RecordingSettings::live_ratio_percent`]: as much as the live traffic (Emilien,
/// 2026-11-24, `decisions.md`). On the simulated `varying-bandwidth` (8 → 1 Mbit/s), the live
/// p99 right after the drop was 80 ms without the upload, 347 ms with no cap, 160 ms at 100 %,
/// 265 ms at 200 %; the upload carried 2.8, 6.3 and 5.4 Mbit/s during the stream.
pub const DEFAULT_LIVE_RATIO_PERCENT: u64 = 100;
/// Default [`RecordingSettings::queue_target_us`]: what the live stream's packets may wait
/// behind the upload. On the simulated network (two 2 Mbit/s links nearly full of live, or two
/// 8 Mbit/s links), 10 ms kept the live p99 within 5 ms of no upload, 30 ms cost it 20–40 ms.
pub const DEFAULT_QUEUE_TARGET_US: u64 = 10_000;
/// Default [`RecordingSettings::base_rtt_window_us`]: long enough for the upload to back off
/// and the queue to drain within it, short enough to follow a handover to a longer path.
pub const DEFAULT_BASE_RTT_WINDOW_US: u64 = 10_000_000;
/// Default [`RecordingSettings::window_gain_chunks`]: from two chunks to a 8 Mbit/s link's
/// bandwidth-delay product at 60 ms (40 chunks) in ten RTTs after a backoff.
pub const DEFAULT_WINDOW_GAIN_CHUNKS: u64 = 4;
/// Default [`RecordingSettings::idle_live_bps`]: above what SRT's control packets weigh without
/// a stream, far below any video.
pub const DEFAULT_IDLE_LIVE_BPS: u64 = 100_000;
/// Default [`RecordingSettings::loss_timeout_us`]: beyond the longest a link holds a packet
/// before the sender calls it stalled (1 s), so a chunk is not resent while it may still arrive.
pub const DEFAULT_LOSS_TIMEOUT_US: u64 = 1_500_000;
/// Default [`RecordingSettings::end_idle_us`]: a camera's file grows several times a second
/// while it records.
pub const DEFAULT_END_IDLE_US: u64 = 2_000_000;
/// Default [`RecordingSettings::drain_us`]: an hour, for a box left on after the stream.
pub const DEFAULT_DRAIN_US: u64 = 3_600_000_000;
/// Default [`RecordingSettings::window_bytes`]: 256 MiB, 7 minutes at 5 Mbit/s of backlog
/// (an outage, or a recording faster than the spare capacity).
pub const DEFAULT_WINDOW_BYTES: u64 = 256 * 1024 * 1024;
/// Default [`RecordingSettings::in_flight_chunks`]: 12 MB with full chunks, a second at
/// 100 Mbit/s.
pub const DEFAULT_IN_FLIGHT_CHUNKS: usize = 8_192;

const MICROSECONDS_PER_MILLISECOND: u64 = 1_000;
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const PERCENT: u64 = 100;

/// How the recording is uploaded. Each field reads from one environment variable (the
/// `*_VARIABLE` constants), and has a default (the `DEFAULT_*` constants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingSettings {
    /// The recording, a file that may still be growing ([`RECORDING_VARIABLE`]).
    pub path: PathBuf,
    /// Recording bytes per datagram, from 1 to [`MAX_RECORDING_CHUNK_LEN`]
    /// ([`CHUNK_BYTES_VARIABLE`], default [`DEFAULT_CHUNK_BYTES`]).
    pub chunk_bytes: usize,
    /// Most of a link's congestion window the upload takes, in per cent of what the live
    /// traffic leaves of it, from 1 to 100 ([`SHARE_PERCENT_VARIABLE`], default [`DEFAULT_SHARE_PERCENT`]).
    pub share_percent: u64,
    /// While the live stream is not idle, the upload's bytes in flight on a link stay under this
    /// share of the live bytes in flight on it, in per cent, from 1: a link whose capacity
    /// drops at once delivers what it holds late, the live packets behind the upload's
    /// included, so what it holds is kept near what step 5 had there. No cap once the live
    /// stream is idle ([`LIVE_RATIO_PERCENT_VARIABLE`], default [`DEFAULT_LIVE_RATIO_PERCENT`]).
    pub live_ratio_percent: u64,
    /// The upload's room on a link shrinks in proportion as its queue grows (smoothed RTT
    /// over its RTT without a queue), none left at this, in microseconds: the live stream's packets
    /// wait behind what it queues
    /// ([`QUEUE_TARGET_MS_VARIABLE`] in milliseconds, default [`DEFAULT_QUEUE_TARGET_US`]).
    pub queue_target_us: u64,
    /// A link's RTT without a queue is its lowest smoothed RTT over the last one to two windows
    /// of this, in microseconds: the queue is the smoothed RTT over it, so jitter does not
    /// read as a queue ([`BASE_RTT_WINDOW_S_VARIABLE`] in seconds, default
    /// [`DEFAULT_BASE_RTT_WINDOW_US`]).
    pub base_rtt_window_us: u64,
    /// Out of slow start, a link's spare window grows by up to this many chunks per RTT while
    /// its queue is under the target, and shrinks as fast beyond it, from 1
    /// ([`WINDOW_GAIN_CHUNKS_VARIABLE`], default [`DEFAULT_WINDOW_GAIN_CHUNKS`]).
    pub window_gain_chunks: u64,
    /// The live stream is idle below this, in bit/s over the last second: then no live packet
    /// waits in a link's queue, and the upload's queue target also allows the link's jitter and
    /// the time to send what one ACK covers, so slow and jittery links fill up
    /// ([`IDLE_LIVE_BPS_VARIABLE`], default [`DEFAULT_IDLE_LIVE_BPS`]).
    pub idle_live_bps: u64,
    /// A chunk not acknowledged this long after it was sent is sent again, in microseconds
    /// ([`LOSS_TIMEOUT_MS_VARIABLE`] in milliseconds, default [`DEFAULT_LOSS_TIMEOUT_US`]).
    pub loss_timeout_us: u64,
    /// The recording has ended once it has not grown for this long, in microseconds: its last
    /// partial chunk then goes ([`END_IDLE_MS_VARIABLE`] in milliseconds, default
    /// [`DEFAULT_END_IDLE_US`]).
    pub end_idle_us: u64,
    /// After the recording has ended, the upload keeps trying this long, in microseconds
    /// ([`DRAIN_S_VARIABLE`] in seconds, default [`DEFAULT_DRAIN_US`]).
    pub drain_us: u64,
    /// The upload sends nothing further than this beyond the first byte not delivered yet, in
    /// bytes: bounds what it tracks ([`WINDOW_BYTES_VARIABLE`], default
    /// [`DEFAULT_WINDOW_BYTES`]).
    pub window_bytes: u64,
    /// Chunks in flight at most, on every link ([`IN_FLIGHT_CHUNKS_VARIABLE`], default
    /// [`DEFAULT_IN_FLIGHT_CHUNKS`]).
    pub in_flight_chunks: usize,
}

impl RecordingSettings {
    /// Settings for `path` with every default.
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self {
            path,
            chunk_bytes: DEFAULT_CHUNK_BYTES,
            share_percent: DEFAULT_SHARE_PERCENT,
            live_ratio_percent: DEFAULT_LIVE_RATIO_PERCENT,
            queue_target_us: DEFAULT_QUEUE_TARGET_US,
            base_rtt_window_us: DEFAULT_BASE_RTT_WINDOW_US,
            window_gain_chunks: DEFAULT_WINDOW_GAIN_CHUNKS,
            idle_live_bps: DEFAULT_IDLE_LIVE_BPS,
            loss_timeout_us: DEFAULT_LOSS_TIMEOUT_US,
            end_idle_us: DEFAULT_END_IDLE_US,
            drain_us: DEFAULT_DRAIN_US,
            window_bytes: DEFAULT_WINDOW_BYTES,
            in_flight_chunks: DEFAULT_IN_FLIGHT_CHUNKS,
        }
    }

    /// Reads the settings from environment variables, through `variable` (`std::env::var` in
    /// the binary): `None` when [`RECORDING_VARIABLE`] is unset or empty (no upload), else the
    /// defaults with what the other variables set. An empty variable counts as unset.
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
        let Some(path) = read(RECORDING_VARIABLE) else {
            return Ok(None);
        };
        let defaults = Self::new(PathBuf::from(path));
        let number = |name: &'static str, low: u64, high: u64, unit: &str| {
            read(name)
                .map(|value| match value.parse::<u64>() {
                    Ok(number) if (low..=high).contains(&number) => Ok(number),
                    _ => Err(SettingError {
                        variable: name,
                        value,
                        expected: format!("{unit} from {low} to {high}"),
                    }),
                })
                .transpose()
        };
        let milliseconds = |name: &'static str, default_us: u64| {
            number(
                name,
                1,
                u64::MAX / MICROSECONDS_PER_MILLISECOND,
                "milliseconds",
            )
            .map(|ms| ms.map_or(default_us, |ms| ms * MICROSECONDS_PER_MILLISECOND))
        };
        let seconds = |name: &'static str, low: u64, default_us: u64| {
            number(
                name,
                low,
                u64::MAX / MICROSECONDS_PER_SECOND,
                "a number of seconds",
            )
            .map(|s| s.map_or(default_us, |s| s * MICROSECONDS_PER_SECOND))
        };
        let chunk_bytes = number(
            CHUNK_BYTES_VARIABLE,
            1,
            MAX_RECORDING_CHUNK_LEN as u64,
            "a number of bytes",
        )?
        .and_then(|bytes| usize::try_from(bytes).ok());
        let in_flight_chunks = number(
            IN_FLIGHT_CHUNKS_VARIABLE,
            1,
            u64::from(u32::MAX),
            "a number of chunks",
        )?
        .and_then(|chunks| usize::try_from(chunks).ok());
        Ok(Some(Self {
            chunk_bytes: chunk_bytes.unwrap_or(defaults.chunk_bytes),
            share_percent: number(SHARE_PERCENT_VARIABLE, 1, PERCENT, "a whole per cent")?
                .unwrap_or(defaults.share_percent),
            live_ratio_percent: number(
                LIVE_RATIO_PERCENT_VARIABLE,
                1,
                u64::from(u32::MAX),
                "a whole per cent",
            )?
            .unwrap_or(defaults.live_ratio_percent),
            queue_target_us: milliseconds(QUEUE_TARGET_MS_VARIABLE, defaults.queue_target_us)?,
            base_rtt_window_us: seconds(
                BASE_RTT_WINDOW_S_VARIABLE,
                1,
                defaults.base_rtt_window_us,
            )?,
            window_gain_chunks: number(
                WINDOW_GAIN_CHUNKS_VARIABLE,
                1,
                u64::from(u32::MAX),
                "a number of chunks",
            )?
            .unwrap_or(defaults.window_gain_chunks),
            idle_live_bps: number(IDLE_LIVE_BPS_VARIABLE, 0, u64::MAX, "bit/s")?
                .unwrap_or(defaults.idle_live_bps),
            loss_timeout_us: milliseconds(LOSS_TIMEOUT_MS_VARIABLE, defaults.loss_timeout_us)?,
            end_idle_us: milliseconds(END_IDLE_MS_VARIABLE, defaults.end_idle_us)?,
            drain_us: seconds(DRAIN_S_VARIABLE, 0, defaults.drain_us)?,
            window_bytes: number(
                WINDOW_BYTES_VARIABLE,
                MAX_RECORDING_CHUNK_LEN as u64,
                u64::MAX,
                "a number of bytes",
            )?
            .unwrap_or(defaults.window_bytes),
            in_flight_chunks: in_flight_chunks.unwrap_or(defaults.in_flight_chunks),
            ..defaults
        }))
    }
}
