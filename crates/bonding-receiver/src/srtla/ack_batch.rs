//! The sequence numbers waiting for the next SRTLA ACK of one link.

use bonding_protocol::srtla::SequenceNumber;

use super::limits::ACK_BATCH_LEN;

/// Sequence numbers of the last data packets that arrived on a link, in arrival order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AckBatch {
    numbers: [SequenceNumber; ACK_BATCH_LEN],
    len: usize,
}

impl AckBatch {
    pub(crate) const fn new() -> Self {
        Self {
            numbers: [SequenceNumber::ZERO; ACK_BATCH_LEN],
            len: 0,
        }
    }

    /// Records one number; when that completes a batch, returns it and starts a new one.
    pub(crate) fn push(
        &mut self,
        number: SequenceNumber,
    ) -> Option<[SequenceNumber; ACK_BATCH_LEN]> {
        let slot = self.numbers.get_mut(self.len)?;
        *slot = number;
        self.len += 1;
        if self.len < ACK_BATCH_LEN {
            return None;
        }
        self.len = 0;
        Some(self.numbers)
    }
}
