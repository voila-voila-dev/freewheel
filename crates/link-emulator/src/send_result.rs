//! The answer of the modem when a packet is handed to it.

/// What happened to a packet handed to [`crate::Link::send`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendResult {
    /// The packet is queued in the modem buffer.
    Queued,
    /// The modem buffer is full: the packet is dropped.
    QueueFull,
}
