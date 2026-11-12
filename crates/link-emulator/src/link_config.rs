//! How one direction of a link behaves.

use crate::{Event, GilbertElliott, Trace};

/// Configuration of one direction of a link.
#[derive(Debug, Clone)]
pub struct LinkConfig {
    /// Delivery opportunities over time.
    pub trace: Trace,
    /// One-way propagation delay added after the bottleneck.
    pub propagation_ms: u32,
    /// Modem buffer size; packets beyond it are dropped on send.
    pub queue_bytes: u32,
    /// Random loss process applied to packets leaving the bottleneck.
    pub loss: GilbertElliott,
    /// Scripted events.
    pub events: Vec<Event>,
}
