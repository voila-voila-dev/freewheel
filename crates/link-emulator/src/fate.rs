//! What finally happened to a packet.

/// The final fate of a packet that left the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Received at `arrival_ms`.
    Delivered {
        /// Time the packet reaches the receiver.
        arrival_ms: u64,
    },
    /// Transmitted but lost on the air interface (still counted as data used).
    Lost,
    /// Thrown away by a modem reboot before transmission.
    Flushed,
}
