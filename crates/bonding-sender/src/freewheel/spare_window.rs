//! How much of the recording one link may have in flight on spare capacity.

/// What sets the recording upload's window on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpareWindowSettings {
    /// The queue it aims under, in microseconds.
    pub(crate) queue_target_us: u64,
    /// Bytes it grows by per RTT at an empty queue, once out of slow start.
    pub(crate) gain_bytes: u64,
    /// Never smaller.
    pub(crate) min_bytes: u64,
    /// Never larger.
    pub(crate) max_bytes: u64,
}

/// The recording upload's window on one link, set by the link's queue as LEDBAT does
/// (RFC 6817). It starts by doubling every RTT (slow start) until the queue first passes the
/// target; then each RTT it grows by up to `gain_bytes` while the queue is under the target, in
/// proportion to the room left, shrinks as much beyond it, and halves (once per RTT) past twice
/// the target. It finds what the link carries without a capacity estimate (which only says what
/// the link was given), and settles at a queue just under the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpareWindow {
    bytes: u64,
    slow_start: bool,
    last_halved_us: Option<u64>,
}

impl Default for SpareWindow {
    fn default() -> Self {
        Self {
            bytes: 0,
            slow_start: true,
            last_halved_us: None,
        }
    }
}

impl SpareWindow {
    /// Bytes of the recording the link may have in flight.
    pub(crate) fn bytes(self, settings: &SpareWindowSettings) -> u64 {
        self.bytes.clamp(
            settings.min_bytes,
            settings.max_bytes.max(settings.min_bytes),
        )
    }

    /// `acknowledged_bytes` of the recording left flight at `now_us`, with the queue at
    /// `queue_us` on a link whose RTT is `rtt_us`.
    pub(crate) fn on_acknowledged(
        &mut self,
        now_us: u64,
        acknowledged_bytes: u64,
        queue_us: u64,
        rtt_us: u64,
        settings: &SpareWindowSettings,
    ) {
        let target_us = settings.queue_target_us.max(1);
        let current = self.bytes(settings);
        // Per RTT, `gain_bytes`: per byte acknowledged, its share of the window.
        let step = |share_us: u64| {
            settings.gain_bytes * acknowledged_bytes * share_us / target_us / current.max(1)
        };
        let next = if queue_us <= target_us {
            if self.slow_start {
                current + acknowledged_bytes
            } else {
                current + step(target_us - queue_us)
            }
        } else {
            self.slow_start = false;
            let halving_due = self
                .last_halved_us
                .is_none_or(|at_us| now_us.saturating_sub(at_us) >= rtt_us);
            if queue_us > 2 * target_us && halving_due {
                self.last_halved_us = Some(now_us);
                current / 2
            } else {
                current.saturating_sub(step((queue_us - target_us).min(target_us)))
            }
        };
        self.bytes = next;
        self.bytes = self.bytes(settings);
    }

    /// The link stalled: start again from the smallest window, in slow start.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}
