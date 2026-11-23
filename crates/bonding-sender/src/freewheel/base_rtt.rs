//! A link's RTT without a queue, as the recording upload reads it.

/// The lowest smoothed RTT of a link over the last one to two windows: its RTT with an empty
/// queue. Jitter averages out in the smoothed RTT, so this sits near the link's mean delay,
/// not at its fastest packet; a queue raises the smoothed RTT above it. Windows let it follow
/// a path that got longer.
// Every field is a time: the unit stays in each name (`code-style.md`).
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct BaseRtt {
    window_started_us: Option<u64>,
    current_min_us: Option<u64>,
    previous_min_us: Option<u64>,
}

impl BaseRtt {
    /// The link's smoothed RTT was `smoothed_rtt_us` at `now_us`; windows last `window_us`.
    pub(crate) fn observe(&mut self, now_us: u64, smoothed_rtt_us: u64, window_us: u64) {
        let started_us = *self.window_started_us.get_or_insert(now_us);
        if now_us.saturating_sub(started_us) >= window_us {
            self.previous_min_us = self.current_min_us;
            self.current_min_us = None;
            self.window_started_us = Some(now_us);
        }
        self.current_min_us = Some(
            self.current_min_us
                .map_or(smoothed_rtt_us, |min_us| min_us.min(smoothed_rtt_us)),
        );
    }

    /// How long the queue holds a packet now: `smoothed_rtt_us` over the base, in
    /// microseconds; `None` before the first observation.
    pub(crate) fn queuing_delay_us(&self, smoothed_rtt_us: u64) -> Option<u64> {
        let base_us = match (self.current_min_us, self.previous_min_us) {
            (Some(current), Some(previous)) => current.min(previous),
            (current, previous) => current.or(previous)?,
        };
        Some(smoothed_rtt_us.saturating_sub(base_us))
    }
}
