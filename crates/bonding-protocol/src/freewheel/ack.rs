//! The acknowledgement: what one link delivered, and the ranges received.

use core::slice::ChunksExact;

use super::encode_error::EncodeError;
use super::header::Header;
use super::lengths::{ACK_HEADER_LEN, HEADER_LEN, MAX_DATAGRAM_LEN, SEQUENCE_RANGE_LEN};
use super::link_timing::LinkTiming;
use super::packet::write_header;
use super::packet_kind::PacketKind;
use super::sequence_range::SequenceRange;

/// An ACK, borrowed from the buffer it was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ack<'a> {
    /// Session and the link it reports on (it may travel on another).
    pub header: Header,
    /// What that link delivered, for the sender's estimates.
    pub timing: LinkTiming,
    /// Sequences received in the session, on any link.
    pub ranges: SequenceRanges<'a>,
}

impl Ack<'_> {
    /// Writes an ACK for `ranges` into `out` and returns its length.
    ///
    /// # Errors
    /// Too many ranges for one datagram, or `out` too small.
    pub fn encode(
        header: Header,
        timing: LinkTiming,
        ranges: &[SequenceRange],
        out: &mut [u8],
    ) -> Result<usize, EncodeError> {
        let len = ACK_HEADER_LEN + ranges.len() * SEQUENCE_RANGE_LEN;
        if len > MAX_DATAGRAM_LEN {
            return Err(EncodeError::TooLong { len });
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        let pairs = write_ack_header(header, timing, out);
        for (pair, range) in pairs.chunks_exact_mut(SEQUENCE_RANGE_LEN).zip(ranges) {
            let (first, last) = pair.split_at_mut(SEQUENCE_RANGE_LEN / 2);
            first.copy_from_slice(&range.first.to_be_bytes());
            last.copy_from_slice(&range.last.to_be_bytes());
        }
        Ok(len)
    }
}

/// Writes the header and timing of an ACK into `out` (at least [`ACK_HEADER_LEN`] long) and
/// returns what follows, where the ranges go.
pub(crate) fn write_ack_header(header: Header, timing: LinkTiming, out: &mut [u8]) -> &mut [u8] {
    let rest = write_header(PacketKind::Ack, header, 0, out);
    let (fields, ranges) = rest.split_at_mut(ACK_HEADER_LEN - HEADER_LEN);
    let words = [
        timing.largest_sequence,
        timing.echoed_sent_at_us,
        timing.ack_delay_us,
        timing.received_bytes,
    ];
    for (slot, word) in fields.chunks_exact_mut(4).zip(words) {
        slot.copy_from_slice(&word.to_be_bytes());
    }
    ranges
}

/// The ranges of an ACK as they are on the wire: whole 8-byte pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SequenceRanges<'a> {
    bytes: &'a [u8],
}

impl<'a> SequenceRanges<'a> {
    /// Wraps the bytes after an ACK's fixed fields; `None` unless they are whole pairs.
    pub(crate) const fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len().is_multiple_of(SEQUENCE_RANGE_LEN) {
            Some(Self { bytes })
        } else {
            None
        }
    }

    /// The ranges, in the order the receiver wrote them.
    pub fn iter(&self) -> impl Iterator<Item = SequenceRange> + 'a {
        let pairs: ChunksExact<'a, u8> = self.bytes.chunks_exact(SEQUENCE_RANGE_LEN);
        pairs.filter_map(|pair| {
            let (first, last) = pair.split_at(SEQUENCE_RANGE_LEN / 2);
            Some(SequenceRange {
                first: u32::from_be_bytes(first.try_into().ok()?),
                last: u32::from_be_bytes(last.try_into().ok()?),
            })
        })
    }

    /// How many ranges there are.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len() / SEQUENCE_RANGE_LEN
    }

    /// `true` when there is none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The ranges as they are on the wire.
    pub(crate) const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}
