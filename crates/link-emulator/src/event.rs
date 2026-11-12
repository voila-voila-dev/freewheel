//! Things that happen to a link over time: outages, handovers, address changes, reboots.

/// Something that happens to a link at a given time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// No service in `[start_ms, end_ms)`; queued packets wait.
    Outage {
        /// First millisecond without service.
        start_ms: u64,
        /// First millisecond with service again.
        end_ms: u64,
    },
    /// Cell change while moving: a short interruption, then capacity ramps back up.
    Handover {
        /// Start of the interruption.
        at_ms: u64,
        /// Interruption length.
        interruption_ms: u64,
        /// Time for capacity to go from zero back to the trace's capacity.
        ramp_ms: u64,
    },
    /// The operator gives the link a new public address (NAT rebinding, new bearer).
    AddressChange {
        /// When the address changes.
        at_ms: u64,
    },
    /// The modem restarts: queued packets are flushed, no service, then a new address.
    ModemReboot {
        /// When the modem goes down.
        at_ms: u64,
        /// How long it stays down.
        down_ms: u64,
    },
}

impl Event {
    pub(crate) fn starts_at(self) -> u64 {
        match self {
            Self::Outage { start_ms, .. } => start_ms,
            Self::Handover { at_ms, .. }
            | Self::AddressChange { at_ms }
            | Self::ModemReboot { at_ms, .. } => at_ms,
        }
    }

    pub(crate) fn changes_address(self) -> bool {
        matches!(self, Self::AddressChange { .. } | Self::ModemReboot { .. })
    }

    /// Share of capacity available at `at_ms` (0 = none, 1 = full).
    pub(crate) fn capacity_share(self, at_ms: u64) -> f64 {
        match self {
            Self::Outage { start_ms, end_ms } if (start_ms..end_ms).contains(&at_ms) => 0.0,
            Self::ModemReboot {
                at_ms: start,
                down_ms,
            } if (start..start + down_ms).contains(&at_ms) => 0.0,
            Self::Handover {
                at_ms: start,
                interruption_ms,
                ramp_ms,
            } => {
                let ramp_start = start + interruption_ms;
                if (start..ramp_start).contains(&at_ms) {
                    0.0
                } else if ramp_ms > 0 && (ramp_start..ramp_start + ramp_ms).contains(&at_ms) {
                    ratio(at_ms - ramp_start, ramp_ms)
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(part: u64, whole: u64) -> f64 {
    part as f64 / whole as f64
}
