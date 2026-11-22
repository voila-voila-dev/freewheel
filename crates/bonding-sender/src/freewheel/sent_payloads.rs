//! The live packets sent lately, kept so a lost one can be sent again.

use bonding_protocol::freewheel::MAX_DATA_PAYLOAD_LEN;

use super::limits::SENT_PAYLOADS_CAPACITY;

/// A ring of the last [`SENT_PAYLOADS_CAPACITY`] payloads, by sequence, with when each was
/// first sent. Allocated once, with the sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentPayloads {
    sequences: Vec<Option<u32>>,
    first_sent_us: Vec<u64>,
    lens: Vec<u16>,
    bytes: Vec<u8>,
}

impl Default for SentPayloads {
    fn default() -> Self {
        Self::new()
    }
}

impl SentPayloads {
    /// An empty ring.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sequences: vec![None; SENT_PAYLOADS_CAPACITY],
            first_sent_us: vec![0; SENT_PAYLOADS_CAPACITY],
            lens: vec![0; SENT_PAYLOADS_CAPACITY],
            bytes: vec![0; SENT_PAYLOADS_CAPACITY * MAX_DATA_PAYLOAD_LEN],
        }
    }

    /// Keeps `payload`, first sent at `now_us` as `sequence`, in place of the packet
    /// [`SENT_PAYLOADS_CAPACITY`] sequences older.
    pub fn keep(&mut self, sequence: u32, now_us: u64, payload: &[u8]) {
        let slot = Self::slot(sequence);
        let start = slot * MAX_DATA_PAYLOAD_LEN;
        let (Some(stored), Some(sent_us), Some(len), Some(bytes)) = (
            self.sequences.get_mut(slot),
            self.first_sent_us.get_mut(slot),
            self.lens.get_mut(slot),
            self.bytes.get_mut(start..start + payload.len()),
        ) else {
            return;
        };
        bytes.copy_from_slice(payload);
        *stored = Some(sequence);
        *sent_us = now_us;
        *len = u16::try_from(payload.len()).unwrap_or(0);
    }

    /// The payload of `sequence` and when it was first sent, if it is still kept.
    #[must_use]
    pub fn get(&self, sequence: u32) -> Option<(u64, &[u8])> {
        let slot = Self::slot(sequence);
        if self.sequences.get(slot).copied().flatten() != Some(sequence) {
            return None;
        }
        let start = slot * MAX_DATA_PAYLOAD_LEN;
        let len = usize::from(self.lens.get(slot).copied()?);
        Some((
            self.first_sent_us.get(slot).copied()?,
            self.bytes.get(start..start + len)?,
        ))
    }

    const fn slot(sequence: u32) -> usize {
        sequence as usize % SENT_PAYLOADS_CAPACITY
    }
}
