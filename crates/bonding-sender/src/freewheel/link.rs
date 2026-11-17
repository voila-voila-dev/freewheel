//! One link of the freewheel sender.

use std::net::Ipv4Addr;

use bonding_protocol::freewheel::LinkId;

use super::in_flight::InFlight;
use super::link_estimate::LinkEstimate;
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
        }
    }

    /// It may carry data now.
    pub(crate) const fn usable(&self) -> bool {
        self.open && self.joined && !self.stalled
    }
}
