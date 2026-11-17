//! What the sender believes about one link: RTT, delivery rate, loss.

use bonding_protocol::freewheel::LinkTiming;

use super::limits::{
    INITIAL_RTT_US, MIN_CONGESTION_WINDOW_BYTES, MIN_RATE_BPS, RATE_SAMPLE_PERIOD_US, RATE_SAMPLES,
};

const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const BITS_PER_BYTE: u64 = 8;
/// Smoothed RTT keeps 7/8 of its value per sample (RFC 6298's gain).
const RTT_KEEP_EIGHTHS: u64 = 7;
/// An RTT sample over this is a clock or echo error, not a measure.
const MAX_PLAUSIBLE_RTT_US: u64 = 10_000_000;
/// Loss is in thousandths; a new sample weighs a quarter.
const LOSS_SCALE: u64 = 1_000;
/// Loss counted at most this high, so a lossy link is penalised, not divided by zero.
const MAX_LOSS_PERMILLE: u64 = 900;
/// Fewer bytes than this between two samples say nothing about loss.
const MIN_LOSS_SAMPLE_BYTES: u64 = 16 * 1_024;
/// The congestion window is this many times the bandwidth-delay product: room to grow.
const WINDOW_GAIN: u64 = 2;

/// Per-link estimates, updated from each ACK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkEstimate {
    smoothed_rtt_us: Option<u64>,
    min_rtt_us: Option<u64>,
    /// Start of the current rate sample: time and the receiver's byte count then.
    anchor: Option<(u64, u32)>,
    /// Bytes taken out of flight since the anchor (delivered or lost).
    left_flight_bytes: u64,
    rate_samples_bps: [u64; RATE_SAMPLES],
    next_rate_sample: usize,
    loss_permille: u64,
}

impl Default for LinkEstimate {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkEstimate {
    /// Nothing measured yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            smoothed_rtt_us: None,
            min_rtt_us: None,
            anchor: None,
            left_flight_bytes: 0,
            rate_samples_bps: [0; RATE_SAMPLES],
            next_rate_sample: 0,
            loss_permille: 0,
        }
    }

    /// An ACK arrived at `now_us`; `left_flight_bytes` were acknowledged by it.
    pub fn on_ack(&mut self, now_us: u64, timing: LinkTiming, left_flight_bytes: u64) {
        let rtt_us = u64::from(
            wrapped_us(now_us)
                .wrapping_sub(timing.echoed_sent_at_us)
                .wrapping_sub(timing.ack_delay_us),
        );
        if rtt_us <= MAX_PLAUSIBLE_RTT_US {
            self.smoothed_rtt_us = Some(self.smoothed_rtt_us.map_or(rtt_us, |smoothed| {
                (smoothed * RTT_KEEP_EIGHTHS + rtt_us) / (RTT_KEEP_EIGHTHS + 1)
            }));
            self.min_rtt_us = Some(self.min_rtt_us.map_or(rtt_us, |min| min.min(rtt_us)));
        }
        self.left_flight_bytes += left_flight_bytes;
        let Some((since_us, since_bytes)) = self.anchor else {
            self.anchor = Some((now_us, timing.received_bytes));
            self.left_flight_bytes = 0;
            return;
        };
        let elapsed_us = now_us.saturating_sub(since_us);
        if elapsed_us < RATE_SAMPLE_PERIOD_US {
            return;
        }
        let delivered_bytes = u64::from(timing.received_bytes.wrapping_sub(since_bytes));
        let rate_bps = delivered_bytes * BITS_PER_BYTE * MICROSECONDS_PER_SECOND / elapsed_us;
        if let Some(slot) = self.rate_samples_bps.get_mut(self.next_rate_sample) {
            *slot = rate_bps;
        }
        self.next_rate_sample = (self.next_rate_sample + 1) % RATE_SAMPLES;
        if self.left_flight_bytes >= MIN_LOSS_SAMPLE_BYTES {
            let delivered = delivered_bytes.min(self.left_flight_bytes);
            let lost_permille =
                (self.left_flight_bytes - delivered) * LOSS_SCALE / self.left_flight_bytes;
            self.loss_permille = (self.loss_permille * 3 + lost_permille) / 4;
        }
        self.anchor = Some((now_us, timing.received_bytes));
        self.left_flight_bytes = 0;
    }

    /// The link stalled: its RTT is forgotten (the queue it measured is gone), rate history
    /// and loss are kept, so a link that comes back gets its share back at once.
    pub fn on_stall(&mut self) {
        self.smoothed_rtt_us = None;
        self.anchor = None;
        self.left_flight_bytes = 0;
    }

    /// Smoothed RTT, or [`INITIAL_RTT_US`] before the first sample.
    #[must_use]
    pub fn smoothed_rtt_us(&self) -> u64 {
        self.smoothed_rtt_us.unwrap_or(INITIAL_RTT_US)
    }

    /// Whether an RTT has been measured since the link joined or stalled.
    #[must_use]
    pub const fn has_rtt(&self) -> bool {
        self.smoothed_rtt_us.is_some()
    }

    /// Highest delivery rate of the last second, at least [`MIN_RATE_BPS`].
    #[must_use]
    pub fn rate_bps(&self) -> u64 {
        self.rate_samples_bps
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .max(MIN_RATE_BPS)
    }

    /// Share of bytes lost, in thousandths, at most 900.
    #[must_use]
    pub fn loss_permille(&self) -> u64 {
        self.loss_permille.min(MAX_LOSS_PERMILLE)
    }

    /// Bytes the link may have in flight at `rate_bps`: twice its bandwidth-delay product, at
    /// least [`MIN_CONGESTION_WINDOW_BYTES`].
    #[must_use]
    pub fn congestion_window_bytes(&self, rate_bps: u64) -> u64 {
        let min_rtt_us = self.min_rtt_us.unwrap_or(INITIAL_RTT_US);
        let bdp_bytes = rate_bps * min_rtt_us / MICROSECONDS_PER_SECOND / BITS_PER_BYTE;
        (WINDOW_GAIN * bdp_bytes).max(MIN_CONGESTION_WINDOW_BYTES)
    }

    /// When a packet of `size_bytes` sent now would arrive, in microseconds from now, with
    /// `in_flight_bytes` ahead of it drained at `rate_bps`: half an RTT, plus draining what is
    /// queued, stretched by the loss (a lost packet arrives late, through SRT's retransmission).
    #[must_use]
    pub fn expected_arrival_us(&self, in_flight_bytes: u64, size_bytes: u64, rate_bps: u64) -> u64 {
        let drain_us = (in_flight_bytes + size_bytes) * BITS_PER_BYTE * MICROSECONDS_PER_SECOND
            / rate_bps.max(1);
        let arrival_us = self.smoothed_rtt_us() / 2 + drain_us;
        arrival_us * LOSS_SCALE / (LOSS_SCALE - self.loss_permille())
    }
}

/// The low 32 bits of a microsecond clock, as the wire carries it.
pub(crate) const fn wrapped_us(now_us: u64) -> u32 {
    (now_us & 0xFFFF_FFFF) as u32
}
