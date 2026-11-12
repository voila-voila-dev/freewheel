//! One registered link: a source address and port of a sender.

use std::net::SocketAddrV4;

use super::ack_batch::AckBatch;

/// A link of a group. Identified by address **and** port: a NAT rebinding is a new link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) address: SocketAddrV4,
    /// When it last sent anything but a registration packet, in receiver seconds.
    pub(crate) last_received_s: u64,
    pub(crate) ack_batch: AckBatch,
}

impl Link {
    pub(crate) const fn new(address: SocketAddrV4, now_s: u64) -> Self {
        Self {
            address,
            last_received_s: now_s,
            ack_batch: AckBatch::new(),
        }
    }
}
