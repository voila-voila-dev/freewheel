//! What an ACK tells the sender about one link.

/// Timing and volume one link delivered, from which the sender estimates its RTT and capacity.
///
/// Times are on the sender's clock, in microseconds, wrapping every 71 minutes: only
/// differences matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinkTiming {
    /// Highest sequence received on this link so far.
    pub largest_sequence: u32,
    /// The send time carried by that packet, echoed back.
    pub echoed_sent_at_us: u32,
    /// How long the receiver held that packet before acknowledging it.
    pub ack_delay_us: u32,
    /// Bytes received on this link since it joined, wrapping.
    pub received_bytes: u32,
}
