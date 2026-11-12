//! An SRT data packet: forwarded unchanged, read only for its sequence number.

use super::lengths::SRT_MIN_LEN;
use super::read::word_at;
use super::sequence_number::SequenceNumber;

/// An SRT data packet (top bit clear), at least [`SRT_MIN_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtData<'a> {
    bytes: &'a [u8],
    sequence_number: SequenceNumber,
}

impl<'a> SrtData<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < SRT_MIN_LEN {
            return None;
        }
        let sequence_number = SequenceNumber::new(word_at(bytes, 0)?)?;
        Some(Self {
            bytes,
            sequence_number,
        })
    }

    /// Its SRT sequence number.
    #[must_use]
    pub const fn sequence_number(&self) -> SequenceNumber {
        self.sequence_number
    }

    /// The whole datagram, to forward as is.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}
