//! An SRT ACK: forwarded unchanged, read for the last acknowledged sequence number.

use super::lengths::{SRT_ACK_MIN_LEN, SRT_MIN_LEN};
use super::read::word_at;
use super::sequence_number::SequenceNumber;

/// An SRT ACK long enough to carry its last acknowledged sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtAck<'a> {
    bytes: &'a [u8],
    last_acknowledged: SequenceNumber,
}

impl<'a> SrtAck<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < SRT_ACK_MIN_LEN {
            return None;
        }
        let last_acknowledged = SequenceNumber::new(word_at(bytes, SRT_MIN_LEN)?)?;
        Some(Self {
            bytes,
            last_acknowledged,
        })
    }

    /// Every data packet before this sequence number has arrived.
    #[must_use]
    pub const fn last_acknowledged(&self) -> SequenceNumber {
        self.last_acknowledged
    }

    /// The whole datagram, to forward as is.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}
