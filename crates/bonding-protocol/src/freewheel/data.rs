//! A data packet: bytes of one traffic class, numbered and timed.

use super::header::Header;
use super::traffic_class::TrafficClass;

/// A data packet, borrowed from the buffer it was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Data<'a> {
    /// Session and the link it travels on.
    pub header: Header,
    /// How it must be delivered.
    pub class: TrafficClass,
    /// Its number in the session, shared by all links and classes, wrapping.
    pub sequence: u32,
    /// When the sender sent it, on the sender's clock, in microseconds, wrapping.
    pub sent_at_us: u32,
    /// The bytes it carries.
    pub payload: &'a [u8],
}
