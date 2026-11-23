//! The encoder's video bitrate, following what the transport says the links carry.
//!
//! Down fast, up slowly: a frame lost costs more than a few seconds at a lower quality.
//! Reports are judged over a window of [`ControllerSettings::window_ms`], not one by one: a
//! keyframe is a burst that queues for a few milliseconds on links that have room to spare.
//! Under pressure (the transport drops at the box, or much of the stream queues in the modems), the bitrate falls to what the links carry with some headroom, and by at
//! least a fifth each time; once calm, it climbs straight back to what the links are measured
//! to carry, or a little at a time beyond. The transport measures what an unsaturated link
//! would carry by probing it with padding (`freewheel-sender`), up to what the encoder's
//! maximum needs ([`ControllerSettings::capacity_for_bps`]); beyond the measure, climbing is
//! probing.

use crate::{Bounds, CapacityReport};

/// How the controller behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerSettings {
    /// Bitrates the encoder may use (video only).
    pub bounds: Bounds,
    /// Bitrate to start from.
    pub initial_bps: u32,
    /// What the stream carries besides the video, in bit/s (audio).
    pub other_bps: u32,
    /// Wire bytes per thousand bytes of stream: MPEG-TS, SRT, transport, UDP/IP headers.
    pub wire_permille: u32,
    /// Share of the measured capacity the stream may use under pressure, in per cent.
    pub headroom_percent: u32,
    /// Reports are added up over this long before the controller judges them, in ms.
    pub window_ms: u64,
    /// Pressure: data dropped at the box over the window, in per cent of what was sent...
    pub dropped_percent: u32,
    /// ...or data sent to arrive late (queuing), in per cent of what was sent.
    pub overflow_percent: u32,
    /// After the first report that carried traffic, pressure is ignored this long, in ms: the
    /// links have no estimate yet and the stream starts with a burst.
    pub startup_grace_ms: u64,
    /// Each decrease takes the bitrate to at most this share of the current one, in per cent.
    pub decrease_percent: u32,
    /// Time without pressure before climbing, in milliseconds...
    pub calm_ms: u64,
    /// ...or this, when the pressure was no usable link at all: an outage is not congestion,
    /// and the links that come back are probed at the rate they had (`freewheel-sender`).
    pub calm_after_outage_ms: u64,
    /// While the measured capacity, with headroom, fits more, a climbing step goes straight to
    /// it; else it adds this share of the current bitrate, in per cent (probing)...
    pub increase_percent: u32,
    /// ...every this many milliseconds.
    pub increase_step_ms: u64,
}

impl ControllerSettings {
    /// The lab's stream: `max_bps` of 720p30 video at most, and `other_bps` beside it (the
    /// audio).
    #[must_use]
    pub const fn lab(max_bps: u32, other_bps: u32) -> Self {
        Self {
            bounds: Bounds {
                min_bps: 300_000,
                max_bps,
            },
            initial_bps: max_bps,
            other_bps,
            // 1.10 for MPEG-TS and frames cut into short packets, 1.04 for the headers.
            wire_permille: 1_140,
            headroom_percent: 80,
            window_ms: 500,
            dropped_percent: 1,
            overflow_percent: 20,
            startup_grace_ms: 3_000,
            decrease_percent: 80,
            calm_ms: 2_000,
            calm_after_outage_ms: 500,
            increase_percent: 5,
            increase_step_ms: 500,
        }
    }

    /// What a stream of `video_bps` costs on the wire, with the rest of the stream and every
    /// header, in bit/s.
    #[must_use]
    pub const fn wire_bps(&self, video_bps: u32) -> u64 {
        (video_bps as u64 + self.other_bps as u64) * self.wire_permille as u64 / 1_000
    }

    /// The capacity the links must be measured at for the controller to let the encoder reach
    /// `video_bps`, headroom included, in bit/s: how far the transport needs to probe.
    #[must_use]
    pub const fn capacity_for_bps(&self, video_bps: u32) -> u64 {
        let headroom_percent = if self.headroom_percent == 0 {
            1
        } else {
            self.headroom_percent as u64
        };
        self.wire_bps(video_bps) * 100 / headroom_percent
    }
}

/// Reports added up since the window started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Window {
    started_ms: u64,
    sent_bps: u64,
    overflow_bps: u64,
    dropped_bps: u64,
    min_capacity_bps: u64,
    no_usable_link: bool,
}

/// The controller: feed it every [`CapacityReport`], set the encoder to what it returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controller {
    settings: ControllerSettings,
    bitrate_bps: u32,
    window: Option<Window>,
    first_traffic_ms: Option<u64>,
    last_pressure_ms: Option<u64>,
    /// The last pressure was no usable link at all.
    last_pressure_was_outage: bool,
    last_increase_ms: Option<u64>,
}

impl Controller {
    /// A controller at its initial bitrate.
    #[must_use]
    pub const fn new(settings: ControllerSettings) -> Self {
        Self {
            settings,
            bitrate_bps: settings.bounds.clamp(settings.initial_bps),
            window: None,
            first_traffic_ms: None,
            last_pressure_ms: None,
            last_pressure_was_outage: false,
            last_increase_ms: None,
        }
    }

    /// The video bitrate to use now, in bit/s.
    #[must_use]
    pub const fn bitrate_bps(&self) -> u32 {
        self.bitrate_bps
    }

    /// A report arrived at `now_ms` (any monotonic clock); returns the video bitrate to use.
    pub fn on_report(&mut self, now_ms: u64, report: &CapacityReport) -> u32 {
        let s = self.settings;
        if report.sent_bps > 0 && self.first_traffic_ms.is_none() {
            self.first_traffic_ms = Some(now_ms);
        }
        let window = self.window.get_or_insert(Window {
            started_ms: now_ms,
            min_capacity_bps: u64::MAX,
            ..Window::default()
        });
        window.sent_bps += report.sent_bps;
        window.overflow_bps += report.overflow_bps;
        window.dropped_bps += report.dropped_bps;
        window.min_capacity_bps = window.min_capacity_bps.min(report.capacity_bps);
        window.no_usable_link |= report.usable_links == 0;
        if now_ms.saturating_sub(window.started_ms) < s.window_ms {
            return self.bitrate_bps;
        }
        let window = *window;
        self.window = None;
        let in_grace = self
            .first_traffic_ms
            .is_none_or(|at_ms| now_ms.saturating_sub(at_ms) < s.startup_grace_ms);
        let dropped = window.dropped_bps * 100 > window.sent_bps * u64::from(s.dropped_percent);
        let overflowed =
            window.overflow_bps * 100 > window.sent_bps * u64::from(s.overflow_percent);
        if window.no_usable_link || (!in_grace && (dropped || overflowed)) {
            self.last_pressure_ms = Some(now_ms);
            self.last_pressure_was_outage = window.no_usable_link;
            let lower = if window.no_usable_link {
                s.bounds.min_bps
            } else {
                self.fitting_bps(window.min_capacity_bps)
                    .min(percent_of(self.bitrate_bps, s.decrease_percent))
            };
            self.bitrate_bps = s.bounds.clamp(lower);
            return self.bitrate_bps;
        }
        let calm_ms = if self.last_pressure_was_outage {
            s.calm_after_outage_ms
        } else {
            s.calm_ms
        };
        let calm = self
            .last_pressure_ms
            .is_none_or(|at_ms| now_ms.saturating_sub(at_ms) >= calm_ms);
        let may_increase = self
            .last_increase_ms
            .is_none_or(|at_ms| now_ms.saturating_sub(at_ms) >= s.increase_step_ms);
        if calm && may_increase && self.bitrate_bps < s.bounds.max_bps {
            let fitting_bps = self.fitting_bps(window.min_capacity_bps);
            // Straight to what the links were measured to carry; past it, a little at a time.
            let climbed = if fitting_bps > self.bitrate_bps {
                fitting_bps
            } else {
                let step = percent_of(self.bitrate_bps, s.increase_percent).max(1);
                self.bitrate_bps.saturating_add(step)
            };
            self.bitrate_bps = s.bounds.clamp(climbed);
            self.last_increase_ms = Some(now_ms);
        }
        self.bitrate_bps
    }

    /// The video bitrate whose stream fits in `capacity_bps` on the wire, with headroom.
    fn fitting_bps(&self, capacity_bps: u64) -> u32 {
        let s = self.settings;
        let wire_budget = capacity_bps * u64::from(s.headroom_percent) / 100;
        let stream_budget = wire_budget * 1_000 / u64::from(s.wire_permille.max(1));
        let video = stream_budget.saturating_sub(u64::from(s.other_bps));
        u32::try_from(video).unwrap_or(u32::MAX)
    }
}

fn percent_of(value: u32, percent: u32) -> u32 {
    u32::try_from(u64::from(value) * u64::from(percent) / 100).unwrap_or(u32::MAX)
}
