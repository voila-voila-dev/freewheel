//! One modem's link, as the sender sees it.

use std::net::Ipv4Addr;

use super::limits::WINDOW_DEFAULT_MILLIPACKETS;
use super::link_activity::LinkActivity;
use super::link_handle::LinkHandle;
use super::packet_log::PacketLog;

/// A link: one source address, one socket, its congestion window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) handle: LinkHandle,
    pub(crate) source: Ipv4Addr,
    /// Its socket is bound; until then it is retried at every housekeeping.
    pub(crate) open: bool,
    pub(crate) activity: LinkActivity,
    /// The second it was last chosen for a datagram from the SRT caller.
    pub(crate) last_chosen_s: Option<u64>,
    pub(crate) window_millipackets: u32,
    pub(crate) in_flight_packets: u32,
    pub(crate) log: PacketLog,
}

impl Link {
    pub(crate) const fn new(handle: LinkHandle, source: Ipv4Addr) -> Self {
        Self {
            handle,
            source,
            open: false,
            activity: LinkActivity::Never,
            last_chosen_s: None,
            window_millipackets: WINDOW_DEFAULT_MILLIPACKETS,
            in_flight_packets: 0,
            log: PacketLog::new(),
        }
    }

    /// Upstream's score: window per packet in flight, plus one (integer division).
    pub(crate) const fn score(&self) -> u32 {
        self.window_millipackets / self.in_flight_packets.saturating_add(1)
    }
}
