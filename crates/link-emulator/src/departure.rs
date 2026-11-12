//! A packet leaving the modem.

use crate::Fate;

/// A packet leaving the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Departure {
    /// Caller-chosen id from [`crate::Link::send`].
    pub id: u64,
    /// Packet size.
    pub bytes: u32,
    /// Time the packet was sent.
    pub sent_ms: u64,
    /// Time it left the modem (transmitted or flushed).
    pub served_ms: u64,
    /// Address generation of the link when it was transmitted (see [`crate::Link::address`]).
    pub address: u32,
    /// What happened to it.
    pub fate: Fate,
}
