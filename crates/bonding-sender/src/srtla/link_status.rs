//! A read-only view of one link, for tests, logs and telemetry.

use std::net::Ipv4Addr;

use super::link_handle::LinkHandle;

/// One link as the sender sees it now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkStatus {
    /// Its name.
    pub handle: LinkHandle,
    /// The address its socket is bound to.
    pub source: Ipv4Addr,
    /// Its socket is bound.
    pub open: bool,
    /// It may carry data: it heard from the receiver within the timeout.
    pub usable: bool,
    /// Congestion window, in thousandths of a packet.
    pub window_millipackets: u32,
    /// Packets sent on it and not acknowledged yet.
    pub in_flight_packets: u32,
}
