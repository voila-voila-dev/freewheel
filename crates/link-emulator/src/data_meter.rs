//! Data used on a SIM, as the operator bills it.

use crate::{Departure, Fate};

/// Data used on a SIM, as the operator bills it: every transmitted byte counts,
/// including lost packets; flushed packets were never sent and do not count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DataMeter {
    /// Bytes sent by the box.
    pub up_bytes: u64,
    /// Bytes received by the box.
    pub down_bytes: u64,
}

impl DataMeter {
    /// Total bytes billed.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.up_bytes + self.down_bytes
    }

    /// Cost in cents for a plan priced `cents_per_gb` (10^9 bytes), rounded up.
    #[must_use]
    pub fn cost_cents(&self, cents_per_gb: u64) -> u64 {
        (self.total_bytes() * cents_per_gb).div_ceil(1_000_000_000)
    }

    pub(crate) fn count(&mut self, departures: &[Departure], up: bool) {
        let bytes: u64 = departures
            .iter()
            .filter(|d| d.fate != Fate::Flushed)
            .map(|d| u64::from(d.bytes))
            .sum();
        if up {
            self.up_bytes += bytes;
        } else {
            self.down_bytes += bytes;
        }
    }
}
