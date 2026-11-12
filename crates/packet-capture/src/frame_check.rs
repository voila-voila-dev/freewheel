//! Frame-by-frame check of the video: what the encoder sent against what the SRT listener got.
//!
//! Both sides are `ffmpeg -c copy -f framemd5` listings: one MD5 per compressed video frame,
//! keyed by its presentation timestamp. A frame is intact when its bytes arrived unchanged.

use std::collections::HashMap;

const DEFAULT_TIME_BASE_HZ: i64 = 90_000;
const MILLISECONDS_PER_SECOND: i64 = 1_000;
const TIME_BASE_PREFIX: &str = "#tb 0:";
const PTS_FIELD: usize = 2;
const HASH_FIELD: usize = 5;

/// One compressed video frame of a `framemd5` listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDigest {
    /// Presentation timestamp, in time base units.
    pub pts: i64,
    /// MD5 of the frame's bytes, in hexadecimal.
    pub hash: String,
}

/// A `framemd5` listing: its frames in order and its time base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDigests {
    /// Frames, in the order they were demuxed.
    pub frames: Vec<FrameDigest>,
    /// Time base, in units per second.
    pub time_base_hz: i64,
}

impl FrameDigests {
    /// Parses `ffmpeg -f framemd5` output. Comment lines are skipped except the time base;
    /// malformed lines are ignored.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut time_base_hz = DEFAULT_TIME_BASE_HZ;
        let mut frames = Vec::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix(TIME_BASE_PREFIX) {
                time_base_hz = parse_time_base(rest).unwrap_or(DEFAULT_TIME_BASE_HZ);
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let (Some(pts), Some(hash)) = (fields.get(PTS_FIELD), fields.get(HASH_FIELD)) else {
                continue;
            };
            let Ok(pts) = pts.parse() else { continue };
            frames.push(FrameDigest {
                pts,
                hash: (*hash).to_owned(),
            });
        }
        Self {
            frames,
            time_base_hz,
        }
    }
}

/// How the received video compares with what was sent, frame by frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCheck {
    /// Frames the encoder sent.
    pub sent: usize,
    /// Sent frames that arrived with exactly the same bytes.
    pub intact: usize,
    /// Sent frames that arrived damaged (same timestamp, different bytes).
    pub corrupted: usize,
    /// Sent frames before the first one that arrived: the stream started late (connection
    /// setup), not lost on the way.
    pub before_start: usize,
    /// Sent frames that never arrived, between the first and the last frame that did.
    pub missing: usize,
    /// Sent frames after the last one that arrived: the end of the stream was cut.
    pub truncated: usize,
    /// Longest run of consecutive sent frames that did not arrive intact, between the first
    /// and the last frame that did, in milliseconds.
    pub longest_gap_ms: i64,
}

impl FrameCheck {
    /// Compares the received frames with the sent ones, by timestamp.
    #[must_use]
    pub fn compare(sent: &FrameDigests, received: &FrameDigests) -> Self {
        let arrived: HashMap<i64, &str> = received
            .frames
            .iter()
            .map(|frame| (frame.pts, frame.hash.as_str()))
            .collect();
        let has_arrived = |frame: &FrameDigest| arrived.contains_key(&frame.pts);
        let first_arrived = sent.frames.iter().position(has_arrived);
        let last_arrived = sent.frames.iter().rposition(has_arrived);
        let frame_ms = frame_duration_ms(sent);
        let mut check = Self {
            sent: sent.frames.len(),
            ..Self::default()
        };
        let mut gap_frames: i64 = 0;
        for (index, frame) in sent.frames.iter().enumerate() {
            let intact = match arrived.get(&frame.pts) {
                Some(hash) if *hash == frame.hash => {
                    check.intact += 1;
                    true
                }
                Some(_) => {
                    check.corrupted += 1;
                    false
                }
                None if first_arrived.is_none_or(|first| index < first) => {
                    check.before_start += 1;
                    continue;
                }
                None if last_arrived.is_some_and(|last| index < last) => {
                    check.missing += 1;
                    false
                }
                None => {
                    check.truncated += 1;
                    continue;
                }
            };
            gap_frames = if intact { 0 } else { gap_frames + 1 };
            check.longest_gap_ms = check.longest_gap_ms.max(gap_frames * frame_ms);
        }
        check
    }

    /// Share of the sent frames that arrived intact, in per cent (100 when nothing was sent).
    #[must_use]
    pub fn intact_percent(&self) -> f64 {
        if self.sent == 0 {
            return 100.0;
        }
        to_f64(self.intact) * 100.0 / to_f64(self.sent)
    }

    /// Frames lost or damaged once the stream had started and before it was cut.
    #[must_use]
    pub const fn damaged(&self) -> usize {
        self.corrupted + self.missing
    }
}

fn parse_time_base(text: &str) -> Option<i64> {
    let (numerator, denominator) = text.trim().split_once('/')?;
    let numerator: i64 = numerator.trim().parse().ok()?;
    let denominator: i64 = denominator.trim().parse().ok()?;
    (numerator > 0).then(|| denominator / numerator)
}

/// Median spacing of the sent timestamps, in milliseconds (at least 1).
fn frame_duration_ms(sent: &FrameDigests) -> i64 {
    let mut steps: Vec<i64> = sent
        .frames
        .windows(2)
        .map(|pair| pair[1].pts - pair[0].pts)
        .filter(|step| *step > 0)
        .collect();
    steps.sort_unstable();
    let step = steps.get(steps.len() / 2).copied().unwrap_or(0);
    (step * MILLISECONDS_PER_SECOND / sent.time_base_hz.max(1)).max(1)
}

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: usize) -> f64 {
    value as f64
}
