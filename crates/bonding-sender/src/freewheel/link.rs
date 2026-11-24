//! One link of the freewheel sender.

use std::net::Ipv4Addr;

use bonding_protocol::freewheel::LinkId;

use super::base_rtt::BaseRtt;
use super::in_flight::InFlight;
use super::limits::WINDOW_FULL_MEMORY_US;
use super::link_estimate::LinkEstimate;
use super::probe::LinkProbe;
use super::spare_window::SpareWindow;
use crate::srtla::LinkHandle;

/// One modem's link: its socket, its place in the session, and what we know of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub(crate) handle: LinkHandle,
    pub(crate) source: Ipv4Addr,
    pub(crate) id: LinkId,
    pub(crate) open: bool,
    pub(crate) last_open_attempt_us: Option<u64>,
    /// The receiver accepted it (or acknowledged it, which implies it).
    pub(crate) joined: bool,
    pub(crate) last_join_us: Option<u64>,
    /// No ACK for too long: carries no data until one comes back.
    pub(crate) stalled: bool,
    pub(crate) last_ack_us: u64,
    pub(crate) last_sent_us: u64,
    /// Last time the scheduler found it with a full congestion window.
    pub(crate) last_window_full_us: Option<u64>,
    pub(crate) estimate: LinkEstimate,
    pub(crate) in_flight: InFlight,
    /// Data bytes ever sent on it, live and padding.
    pub(crate) sent_bytes: u64,
    pub(crate) probe: LinkProbe,
    /// Its RTT without a queue, for the recording upload's room.
    pub(crate) base_rtt: BaseRtt,
    /// The recording upload's window on it.
    pub(crate) spare_window: SpareWindow,
    /// Last ACK that found nothing in flight or took packets out of flight: ACKs passing
    /// packets in flight for long without taking them out mean they are lost.
    pub(crate) in_flight_progress_us: u64,
}

impl Link {
    pub(crate) fn new(handle: LinkHandle, source: Ipv4Addr, id: LinkId) -> Self {
        Self {
            handle,
            source,
            id,
            open: false,
            last_open_attempt_us: None,
            joined: false,
            last_join_us: None,
            stalled: false,
            last_ack_us: 0,
            last_sent_us: 0,
            last_window_full_us: None,
            estimate: LinkEstimate::new(),
            in_flight: InFlight::new(),
            sent_bytes: 0,
            probe: LinkProbe::default(),
            base_rtt: BaseRtt::default(),
            spare_window: SpareWindow::default(),
            in_flight_progress_us: 0,
        }
    }

    /// The scheduler found its congestion window full within [`WINDOW_FULL_MEMORY_US`]: what
    /// it delivers is what it can carry.
    pub(crate) fn proven_full(&self, now_us: u64) -> bool {
        self.last_window_full_us
            .is_some_and(|at_us| now_us.saturating_sub(at_us) < WINDOW_FULL_MEMORY_US)
    }

    /// What it is estimated to carry, in bit/s: its delivery rate, or what probing measured if
    /// more and the link has not been found full since.
    pub(crate) fn capacity_bps(&self, now_us: u64) -> u64 {
        let rate_bps = self.estimate.rate_bps();
        if self.proven_full(now_us) {
            rate_bps
        } else {
            rate_bps.max(self.probe.probed_bps(now_us))
        }
    }

    /// It may carry data now.
    pub(crate) const fn usable(&self) -> bool {
        self.open && self.joined && !self.stalled
    }
}
