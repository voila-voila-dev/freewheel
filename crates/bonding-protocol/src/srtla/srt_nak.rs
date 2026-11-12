//! An SRT NAK: forwarded unchanged, read for its loss list.

use core::slice::ChunksExact;

use super::lengths::{SEQUENCE_NUMBER_LEN, SRT_MIN_LEN};
use super::loss_range::LossRange;
use super::sequence_number::{MAX_SEQUENCE_NUMBER, SequenceNumber};

const RANGE_START_BIT: u32 = 1 << 31;

/// An SRT NAK whose loss list is well formed: whole words, every range closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtNak<'a> {
    bytes: &'a [u8],
}

impl<'a> SrtNak<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        let list = bytes.get(SRT_MIN_LEN..)?;
        if list.len() % SEQUENCE_NUMBER_LEN != 0 {
            return None;
        }
        let mut words = LossRanges::over(list);
        while let Some(range) = words.next_checked() {
            range.ok()?;
        }
        Some(Self { bytes })
    }

    /// The lost packets, in the order the listener reported them.
    #[must_use]
    pub fn loss_ranges(&self) -> LossRanges<'a> {
        LossRanges::over(self.bytes.get(SRT_MIN_LEN..).unwrap_or_default())
    }

    /// The whole datagram, to forward as is.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// A range start not followed by a 31-bit end.
#[derive(Debug, Clone, Copy)]
struct MalformedLossList;

/// Iterator over the [`LossRange`]s of an [`SrtNak`].
#[derive(Debug, Clone)]
pub struct LossRanges<'a> {
    words: ChunksExact<'a, u8>,
}

impl<'a> LossRanges<'a> {
    fn over(list: &'a [u8]) -> Self {
        Self {
            words: list.chunks_exact(SEQUENCE_NUMBER_LEN),
        }
    }

    fn next_word(&mut self) -> Option<u32> {
        let word: [u8; SEQUENCE_NUMBER_LEN] = self.words.next()?.try_into().ok()?;
        Some(u32::from_be_bytes(word))
    }

    /// `None` at the end of the list.
    fn next_checked(&mut self) -> Option<Result<LossRange, MalformedLossList>> {
        let word = self.next_word()?;
        if word & RANGE_START_BIT == 0 {
            let lost = SequenceNumber::new(word).ok_or(MalformedLossList);
            return Some(lost.map(|lost| LossRange {
                first: lost,
                last: lost,
            }));
        }
        let first = SequenceNumber::new(word & MAX_SEQUENCE_NUMBER);
        let last = self.next_word().and_then(SequenceNumber::new);
        Some(
            first
                .zip(last)
                .map(|(first, last)| LossRange { first, last })
                .ok_or(MalformedLossList),
        )
    }
}

impl Iterator for LossRanges<'_> {
    type Item = LossRange;

    fn next(&mut self) -> Option<LossRange> {
        self.next_checked()?.ok()
    }
}
