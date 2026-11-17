//! One link of a session, as the receiver sees it.

use std::net::SocketAddrV4;

use bonding_protocol::freewheel::{LinkId, LinkTiming};

/// What the receiver knows about one link: where it is, and what to acknowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub(crate) id: LinkId,
    /// Where its last packet came from; follows NAT rebinding and IP changes.
    pub(crate) address: SocketAddrV4,
    pub(crate) last_received_us: u64,
    /// Sequence of the last data packet that arrived.
    pub(crate) latest_sequence: u32,
    /// Send time of the last packet that arrived (data or keepalive), to echo.
    pub(crate) latest_sent_at_us: u32,
    pub(crate) latest_arrival_us: u64,
    /// Bytes received since it joined, wrapping.
    pub(crate) received_bytes: u32,
    /// Packets received since the last ACK.
    pub(crate) unacknowledged_packets: u32,
}

impl Link {
    pub(crate) const fn new(id: LinkId, address: SocketAddrV4, now_us: u64) -> Self {
        Self {
            id,
            address,
            last_received_us: now_us,
            latest_sequence: 0,
            latest_sent_at_us: 0,
            latest_arrival_us: now_us,
            received_bytes: 0,
            unacknowledged_packets: 0,
        }
    }

    /// What its next ACK says, at `now_us`.
    pub(crate) fn timing(&self, now_us: u64) -> LinkTiming {
        let held_us = now_us.saturating_sub(self.latest_arrival_us);
        LinkTiming {
            largest_sequence: self.latest_sequence,
            echoed_sent_at_us: self.latest_sent_at_us,
            ack_delay_us: u32::try_from(held_us).unwrap_or(u32::MAX),
            received_bytes: self.received_bytes,
        }
    }
}
