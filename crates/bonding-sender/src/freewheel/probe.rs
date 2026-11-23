//! Capacity probing: padding on a link's spare capacity, so the transport knows what each link
//! carries, not only what it was given (M1b step 5, `docs/technical/protocol/freewheel.md`,
//! "Capacity probing").
//!
//! A link only measures what it is given: once the encoder has lowered its bitrate, the links
//! report that bitrate and the encoder has nothing to climb back to. So, while the encoder is
//! below its maximum and the links are not known to carry what that maximum needs, each link
//! with spare room is sent short runs of padding ([`PROBE_DURATION_US`]) at twice what it is
//! known to carry, at most once per [`PROBE_INTERVAL_US`], and only once the budget holds the
//! whole run; what it delivered meanwhile is its probed rate, kept [`PROBED_RATE_MEMORY_US`]
//! or until the link is found full. A link that comes back after a stall is probed at once, at
//! the rate it had. Padding never exceeds [`PROBE_BUDGET_PERCENT`] of the estimated capacity,
//! on average.

/// What the encoder's maximum needs: probing stops once the links are known to carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeTarget {
    /// The stream at the encoder's maximum bitrate, on the wire, in bit/s: live traffic at
    /// [`AT_MAXIMUM_PERCENT`] of it or more means the encoder is at its maximum.
    pub stream_max_wire_bps: u64,
    /// The capacity the encoder's controller needs to reach its maximum, headroom included
    /// (`bitrate_control::ControllerSettings::capacity_for_bps`), in bit/s.
    pub wanted_capacity_bps: u64,
}

/// A run of padding lasts this long, in microseconds.
pub const PROBE_DURATION_US: u64 = 150_000;
/// A run's delivery rate is measured between the first ACK of its padding and a later one at
/// least this long after (the receiver's byte count over the ACKs' spacing): shorter, ACKs
/// bunched by the way back would read as a faster link.
pub const PROBE_MIN_SAMPLE_US: u64 = 50_000;
/// What delivered within this long after a run still counts for it (the last padding is still
/// in flight), in microseconds.
pub const PROBE_SETTLE_US: u64 = 200_000;
/// A link is probed at most this often, in microseconds.
pub const PROBE_INTERVAL_US: u64 = 1_000_000;
/// A run aims at this many times what the link is known to carry: doubling, so 0.5 to
/// 8 Mbit/s takes four runs.
pub const PROBE_GAIN: u64 = 2;
/// A probed rate counts this long, in microseconds, unless the link is found full before
/// (then what it delivers is the measure): probing stops once the links are known to carry
/// enough, and resumes when this lapses.
pub const PROBED_RATE_MEMORY_US: u64 = 10_000_000;
/// Padding stays under this share of the estimated capacity, in per cent, on average...
pub const PROBE_BUDGET_PERCENT: u64 = 10;
/// ...with at most this long of unspent budget kept, in microseconds: enough to probe every
/// link at once when they come back after an outage.
pub const PROBE_BUDGET_DEPTH_US: u64 = 2_000_000;
/// Live traffic at this share of the stream's maximum, or more, means the encoder is at its
/// maximum: nothing to probe for, in per cent.
pub const AT_MAXIMUM_PERCENT: u64 = 90;
/// The live traffic is the median of this many capacity reports (a second): a keyframe, the
/// frames between two, or the first report of the stream do not decide.
const LIVE_REPORTS: usize = 10;

const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const BITS_PER_BYTE: u64 = 8;

/// One run of padding on one link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeRun {
    pub(crate) started_us: u64,
    pub(crate) target_bps: u64,
    /// The link's data bytes sent when the run started: the run tops them up to its target.
    pub(crate) sent_bytes_at_start: u64,
}

/// When the first ACK of a run's padding arrived, and the receiver's byte count it carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DeliveryAnchor {
    at_us: u64,
    received_bytes: u32,
}

/// What probing knows and does on one link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LinkProbe {
    pub(crate) run: Option<ProbeRun>,
    /// ACKs until then count for the last run.
    measuring_until_us: Option<u64>,
    /// Sequence of the run's first padding packet.
    first_sequence: Option<u32>,
    anchor: Option<DeliveryAnchor>,
    best_sample_bps: u64,
    probed_bps: u64,
    probed_at_us: Option<u64>,
    /// What the link carried before it stalled: the first run after it comes back aims there.
    pub(crate) known_bps: Option<u64>,
    last_started_us: Option<u64>,
}

impl LinkProbe {
    /// A run is going on, or its padding is still in flight: the link's window is full on
    /// purpose, which says nothing about its capacity.
    pub(crate) const fn busy(&self) -> bool {
        self.run.is_some() || self.measuring_until_us.is_some()
    }

    /// The rate the last runs measured, if recent enough.
    pub(crate) fn probed_bps(&self, now_us: u64) -> u64 {
        match self.probed_at_us {
            Some(at_us) if now_us.saturating_sub(at_us) < PROBED_RATE_MEMORY_US => self.probed_bps,
            _ => 0,
        }
    }

    /// The link was found full: what it delivers is the measure, the probed rate is stale.
    pub(crate) const fn forget_probed(&mut self) {
        self.probed_at_us = None;
    }

    /// A padding packet of the run went out as `sequence`.
    pub(crate) fn on_padding_sent(&mut self, sequence: u32) {
        if self.first_sequence.is_none() {
            self.first_sequence = Some(sequence);
        }
    }

    /// An ACK arrived at `now_us` on this link, up to `largest_sequence`, the receiver having
    /// got `received_bytes` on it so far (wrapping): measures the run's delivery rate.
    pub(crate) fn on_ack(&mut self, now_us: u64, largest_sequence: u32, received_bytes: u32) {
        if !self.busy() {
            return;
        }
        let Some(first_sequence) = self.first_sequence else {
            return;
        };
        let Some(anchor) = self.anchor else {
            // Sequences wrap: "at or after the first padding" within half the space.
            if largest_sequence.wrapping_sub(first_sequence) < u32::MAX / 2 {
                self.anchor = Some(DeliveryAnchor {
                    at_us: now_us,
                    received_bytes,
                });
            }
            return;
        };
        let elapsed_us = now_us.saturating_sub(anchor.at_us);
        if elapsed_us < PROBE_MIN_SAMPLE_US {
            return;
        }
        let delivered_bytes = u64::from(received_bytes.wrapping_sub(anchor.received_bytes));
        let rate_bps = delivered_bytes * BITS_PER_BYTE * MICROSECONDS_PER_SECOND / elapsed_us;
        self.best_sample_bps = self.best_sample_bps.max(rate_bps);
    }

    /// Whether a run may start now (the link is usable, measured, and not found full).
    pub(crate) fn due(&self, now_us: u64) -> bool {
        !self.busy()
            && (self.known_bps.is_some()
                || self
                    .last_started_us
                    .is_none_or(|at_us| now_us.saturating_sub(at_us) >= PROBE_INTERVAL_US))
    }

    /// Starts a run at `target_bps`.
    pub(crate) fn start(&mut self, now_us: u64, target_bps: u64, sent_bytes: u64) {
        self.known_bps = None;
        self.last_started_us = Some(now_us);
        self.best_sample_bps = 0;
        self.first_sequence = None;
        self.anchor = None;
        self.run = Some(ProbeRun {
            started_us: now_us,
            target_bps,
            sent_bytes_at_start: sent_bytes,
        });
    }

    /// Ends a run that has lasted [`PROBE_DURATION_US`], and records what a finished run
    /// measured once its padding has settled.
    pub(crate) fn advance(&mut self, now_us: u64) {
        if let Some(run) = self.run
            && now_us.saturating_sub(run.started_us) >= PROBE_DURATION_US
        {
            self.run = None;
            self.measuring_until_us = Some(now_us + PROBE_SETTLE_US);
        }
        if self
            .measuring_until_us
            .is_some_and(|until_us| now_us >= until_us)
        {
            self.measuring_until_us = None;
            if self.best_sample_bps > 0 {
                self.probed_bps = self.best_sample_bps;
                self.probed_at_us = Some(now_us);
            }
            self.best_sample_bps = 0;
        }
    }

    /// Bytes the run still owes the link at `now_us`, given what the link has sent so far.
    pub(crate) fn owed_bytes(&self, now_us: u64, sent_bytes: u64) -> u64 {
        let Some(run) = self.run else { return 0 };
        let due_bytes = run.target_bps * now_us.saturating_sub(run.started_us)
            / MICROSECONDS_PER_SECOND
            / BITS_PER_BYTE;
        due_bytes.saturating_sub(sent_bytes.saturating_sub(run.sent_bytes_at_start))
    }

    /// The link stalled: the run stops, what it carried is remembered for when it returns.
    pub(crate) fn on_stall(&mut self, carried_bps: u64) {
        self.run = None;
        self.measuring_until_us = None;
        self.first_sequence = None;
        self.anchor = None;
        self.best_sample_bps = 0;
        self.probed_at_us = None;
        self.known_bps = Some(carried_bps);
    }
}

/// Probing for the whole sender: the target, the padding budget, the live traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Probing {
    pub(crate) target: ProbeTarget,
    pub(crate) budget_bytes: u64,
    last_refill_us: Option<u64>,
    /// Live traffic of the last [`LIVE_REPORTS`] capacity reports, in bit/s on the wire,
    /// since the first that carried some.
    live_bps: [u64; LIVE_REPORTS],
    live_reports: usize,
}

impl Probing {
    pub(crate) const fn new(target: ProbeTarget) -> Self {
        Self {
            target,
            budget_bytes: 0,
            last_refill_us: None,
            live_bps: [0; LIVE_REPORTS],
            live_reports: 0,
        }
    }

    /// Adds the budget earned since the last call, at [`PROBE_BUDGET_PERCENT`] of
    /// `capacity_bps`.
    pub(crate) fn refill(&mut self, now_us: u64, capacity_bps: u64) {
        let elapsed_us = self
            .last_refill_us
            .map_or(0, |at_us| now_us.saturating_sub(at_us));
        self.last_refill_us = Some(now_us);
        let budget_bps = capacity_bps * PROBE_BUDGET_PERCENT / 100;
        let earned = budget_bps * elapsed_us / MICROSECONDS_PER_SECOND / BITS_PER_BYTE;
        let depth = budget_bps * PROBE_BUDGET_DEPTH_US / MICROSECONDS_PER_SECOND / BITS_PER_BYTE;
        // A budget earned while the capacity was higher is kept: it pays for probing the
        // links back to where they were after an outage.
        if self.budget_bytes < depth {
            self.budget_bytes = (self.budget_bytes + earned).min(depth);
        }
    }

    /// Padding a run at `target_bps` costs on a link carrying `carried_bps`, in bytes.
    pub(crate) const fn run_cost_bytes(target_bps: u64, carried_bps: u64) -> u64 {
        target_bps.saturating_sub(carried_bps) * PROBE_DURATION_US
            / MICROSECONDS_PER_SECOND
            / BITS_PER_BYTE
    }

    /// A capacity report measured `sent_bps` of live traffic.
    pub(crate) fn on_live(&mut self, sent_bps: u64) {
        if self.live_reports == 0 && sent_bps == 0 {
            return;
        }
        if let Some(slot) = self.live_bps.get_mut(self.live_reports % LIVE_REPORTS) {
            *slot = sent_bps;
        }
        self.live_reports = self.live_reports.saturating_add(1);
    }

    /// The stream has run a second and the encoder is below its maximum: there is something
    /// to probe for.
    pub(crate) fn below_maximum(&self) -> bool {
        if self.live_reports < LIVE_REPORTS {
            return false;
        }
        let mut sorted = self.live_bps;
        sorted.sort_unstable();
        let median_bps = sorted.get(LIVE_REPORTS / 2).copied().unwrap_or(0);
        median_bps * 100 < self.target.stream_max_wire_bps * AT_MAXIMUM_PERCENT
    }

    /// The links are not known to carry what the encoder's maximum needs.
    pub(crate) const fn wanted(&self, capacity_bps: u64) -> bool {
        capacity_bps < self.target.wanted_capacity_bps
    }
}
