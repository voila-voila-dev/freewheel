//! How long the video took from the encoder to the server, frame by frame.

use std::collections::{HashMap, HashSet};

use crate::{FrameTime, percentile_of_sorted};

const MICROSECONDS_PER_MILLISECOND: f64 = 1_000.0;
const MEDIAN_PERCENT: f64 = 50.0;
const TAIL_PERCENT: f64 = 99.0;

/// Latency from the encoder's output to the SRT listener's output, over the frames that arrived
/// intact: what the bonding and SRT's buffer add, both clocks being the lab host's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameLatency {
    /// Frames measured (sent, and delivered intact).
    pub frames: usize,
    /// Median latency, in milliseconds.
    pub median_ms: f64,
    /// 99th percentile, in milliseconds.
    pub p99_ms: f64,
    /// Worst frame, in milliseconds.
    pub max_ms: f64,
}

impl FrameLatency {
    /// Pairs each sent frame with its first delivery, by timestamp, keeping only the timestamps
    /// in `intact` (frames that arrived with exactly the bytes sent). `None` when no frame pairs.
    #[must_use]
    pub fn measure(
        sent: &[FrameTime],
        received: &[FrameTime],
        intact: &HashSet<i64>,
    ) -> Option<Self> {
        let mut delivered: HashMap<i64, u64> = HashMap::with_capacity(received.len());
        for frame in received {
            delivered.entry(frame.pts).or_insert(frame.at_us);
        }
        let mut latencies_ms: Vec<f64> = sent
            .iter()
            .filter(|frame| intact.contains(&frame.pts))
            .filter_map(|frame| {
                let at_us = *delivered.get(&frame.pts)?;
                Some(to_f64(at_us) - to_f64(frame.at_us))
            })
            .map(|microseconds| microseconds / MICROSECONDS_PER_MILLISECOND)
            .collect();
        if latencies_ms.is_empty() {
            return None;
        }
        latencies_ms.sort_by(f64::total_cmp);
        Some(Self {
            frames: latencies_ms.len(),
            median_ms: percentile_of_sorted(&latencies_ms, MEDIAN_PERCENT),
            p99_ms: percentile_of_sorted(&latencies_ms, TAIL_PERCENT),
            max_ms: latencies_ms[latencies_ms.len() - 1],
        })
    }
}

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: u64) -> f64 {
    value as f64
}
