//! The recording chunks a session received last, as the ranges its ACKs carry.

use bonding_protocol::freewheel::SequenceRange;

use super::recording_settings::MAX_ACKNOWLEDGED_RANGES;

/// The sequences of the last recording chunks a session received and kept, merged into runs
/// ([`SequenceRange`]), oldest first, at most `limit` of them. Every ACK of the session carries
/// them, on any link: each chunk is acknowledged by several ACKs, so a lost ACK or a link that
/// dies right after costs no resend. Fixed size, no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentChunks {
    ranges: [SequenceRange; MAX_ACKNOWLEDGED_RANGES],
    oldest: usize,
    count: usize,
    limit: usize,
}

impl RecentChunks {
    /// None yet; keeps at most `limit` ranges (1 to [`MAX_ACKNOWLEDGED_RANGES`]).
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            ranges: [SequenceRange::default(); MAX_ACKNOWLEDGED_RANGES],
            oldest: 0,
            count: 0,
            limit: limit.clamp(1, MAX_ACKNOWLEDGED_RANGES),
        }
    }

    /// The chunk numbered `sequence` was received and kept: it extends the newest run when it
    /// follows it, else starts a run (the oldest is forgotten when `limit` are kept).
    pub(crate) fn record(&mut self, sequence: u32) {
        if self.count > 0 {
            let newest = (self.oldest + self.count - 1) % MAX_ACKNOWLEDGED_RANGES;
            if let Some(range) = self.ranges.get_mut(newest) {
                // Sequences wrap: "in the run" is within it counting from its first.
                if sequence.wrapping_sub(range.first) <= range.last.wrapping_sub(range.first) {
                    return;
                }
                if range.last.wrapping_add(1) == sequence {
                    range.last = sequence;
                    return;
                }
            }
        }
        if self.count == self.limit {
            self.oldest = (self.oldest + 1) % MAX_ACKNOWLEDGED_RANGES;
            self.count -= 1;
        }
        let slot = (self.oldest + self.count) % MAX_ACKNOWLEDGED_RANGES;
        if let Some(range) = self.ranges.get_mut(slot) {
            *range = SequenceRange {
                first: sequence,
                last: sequence,
            };
            self.count += 1;
        }
    }

    /// Copies the runs into `out`, oldest first, and returns them.
    pub(crate) fn copy_into<'a>(
        &self,
        out: &'a mut [SequenceRange; MAX_ACKNOWLEDGED_RANGES],
    ) -> &'a [SequenceRange] {
        for (index, slot) in out.iter_mut().take(self.count).enumerate() {
            if let Some(range) = self
                .ranges
                .get((self.oldest + index) % MAX_ACKNOWLEDGED_RANGES)
            {
                *slot = *range;
            }
        }
        out.get(..self.count).unwrap_or_default()
    }
}
