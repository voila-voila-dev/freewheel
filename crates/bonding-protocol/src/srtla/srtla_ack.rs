//! The SRTLA ACK: sequence numbers of data packets that arrived on one link.

use core::slice::ChunksExact;

use super::encode_error::EncodeError;
use super::lengths::{MAX_DATAGRAM_LEN, SEQUENCE_NUMBER_LEN, SRTLA_ACK_HEADER_LEN, TYPE_LEN};
use super::packet_type::PacketType;
use super::sequence_number::SequenceNumber;

/// A well-formed SRTLA ACK: zero padding after the type, then whole sequence numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtlaAck<'a> {
    sequence_numbers: &'a [u8],
}

impl<'a> SrtlaAck<'a> {
    pub(crate) fn parse(packet: &'a [u8]) -> Option<Self> {
        let padding = packet.get(TYPE_LEN..SRTLA_ACK_HEADER_LEN)?;
        let sequence_numbers = packet.get(SRTLA_ACK_HEADER_LEN..)?;
        let well_formed = padding.iter().all(|&byte| byte == 0)
            && sequence_numbers.len() % SEQUENCE_NUMBER_LEN == 0
            && Self::words(sequence_numbers).all(|word| SequenceNumber::new(word).is_some());
        well_formed.then_some(Self { sequence_numbers })
    }

    /// Writes an SRTLA ACK for `sequence_numbers` into `out` and returns its length.
    ///
    /// # Errors
    /// Too many numbers for one datagram, or `out` too small.
    pub fn encode(
        sequence_numbers: &[SequenceNumber],
        out: &mut [u8],
    ) -> Result<usize, EncodeError> {
        let len = SRTLA_ACK_HEADER_LEN + sequence_numbers.len() * SEQUENCE_NUMBER_LEN;
        if len > MAX_DATAGRAM_LEN {
            return Err(EncodeError::TooLong { len });
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        let (header, words) = out.split_at_mut(SRTLA_ACK_HEADER_LEN);
        header.copy_from_slice(&(u32::from(PacketType::SrtlaAck.raw()) << 16).to_be_bytes());
        for (word, number) in words
            .chunks_exact_mut(SEQUENCE_NUMBER_LEN)
            .zip(sequence_numbers)
        {
            word.copy_from_slice(&number.get().to_be_bytes());
        }
        Ok(len)
    }

    /// The acknowledged sequence numbers, in the order the receiver logged them.
    pub fn sequence_numbers(&self) -> impl Iterator<Item = SequenceNumber> + 'a {
        Self::words(self.sequence_numbers).filter_map(SequenceNumber::new)
    }

    /// How many sequence numbers it carries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.sequence_numbers.len() / SEQUENCE_NUMBER_LEN
    }

    /// `true` when it acknowledges nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.sequence_numbers.is_empty()
    }

    /// The sequence numbers as they are on the wire.
    pub(crate) const fn sequence_number_bytes(&self) -> &'a [u8] {
        self.sequence_numbers
    }

    fn words(bytes: &'a [u8]) -> impl Iterator<Item = u32> + 'a {
        let chunks: ChunksExact<'a, u8> = bytes.chunks_exact(SEQUENCE_NUMBER_LEN);
        chunks.filter_map(|chunk| Some(u32::from_be_bytes(chunk.try_into().ok()?)))
    }
}
