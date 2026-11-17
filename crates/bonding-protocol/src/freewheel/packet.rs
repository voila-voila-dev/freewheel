//! Every freewheel datagram, decoded and encoded.

use super::ack::{Ack, SequenceRanges, write_ack_header};
use super::close_reason::CloseReason;
use super::data::Data;
use super::decode_error::DecodeError;
use super::encode_error::EncodeError;
use super::header::Header;
use super::lengths::{
    ACCEPT_LEN, ACK_HEADER_LEN, CLOSE_LEN, DATA_HEADER_LEN, HEADER_LEN, JOIN_LEN, KEEPALIVE_LEN,
    MAGIC, MAX_DATAGRAM_LEN,
};
use super::link_timing::LinkTiming;
use super::packet_kind::PacketKind;
use super::read::{u32_at, u64_at};
use super::traffic_class::TrafficClass;

const KIND_OFFSET: usize = 1;
const LINK_OFFSET: usize = 2;
const CLASS_OFFSET: usize = 3;
const SESSION_OFFSET: usize = 4;

/// `true` when a datagram is ours (starts with [`MAGIC`]), so the server can tell it from
/// SRT and SRTLA on the same port with one byte.
#[must_use]
pub fn is_freewheel(bytes: &[u8]) -> bool {
    bytes.first() == Some(&MAGIC)
}

/// One freewheel datagram, borrowed from the buffer it was read from.
///
/// Decoding then encoding gives back the same bytes: every field is kept, and anything v0
/// does not define is refused rather than ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packet<'a> {
    /// Bytes of one traffic class.
    Data(Data<'a>),
    /// What one link delivered.
    Ack(Ack<'a>),
    /// Keeps an idle link measured; answered by an ACK echoing its send time.
    Keepalive {
        /// Session and link.
        header: Header,
        /// When it was sent, on the sender's clock, in microseconds, wrapping.
        sent_at_us: u32,
    },
    /// A link asks to join a session.
    Join(Header),
    /// The server accepts the link.
    Accept(Header),
    /// The link (or the session) ends.
    Close {
        /// Session and link.
        header: Header,
        /// Why.
        reason: CloseReason,
    },
}

impl<'a> Packet<'a> {
    /// Reads one datagram.
    ///
    /// # Errors
    /// See [`DecodeError`].
    pub fn decode(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let len = bytes.len();
        if len == 0 {
            return Err(DecodeError::TooShort { len });
        }
        if !is_freewheel(bytes) {
            return Err(DecodeError::NotFreewheel);
        }
        if len > MAX_DATAGRAM_LEN {
            return Err(DecodeError::TooLong { len });
        }
        let (Some(&raw_kind), Some(&link_id), Some(&raw_class), Some(session_id)) = (
            bytes.get(KIND_OFFSET),
            bytes.get(LINK_OFFSET),
            bytes.get(CLASS_OFFSET),
            u64_at(bytes, SESSION_OFFSET),
        ) else {
            return Err(DecodeError::TooShort { len });
        };
        let kind =
            PacketKind::from_raw(raw_kind).ok_or(DecodeError::UnknownKind { raw: raw_kind })?;
        let header = Header {
            session_id,
            link_id,
        };
        if kind != PacketKind::Data && raw_class != 0 {
            return Err(DecodeError::ReservedNotZero);
        }
        let exact = |expected: usize| {
            if len == expected {
                Ok(())
            } else {
                Err(DecodeError::WrongLength { kind, len })
            }
        };
        match kind {
            PacketKind::Data => Self::decode_data(header, raw_class, bytes),
            PacketKind::Ack => Self::decode_ack(header, bytes),
            PacketKind::Keepalive => {
                exact(KEEPALIVE_LEN)?;
                let sent_at_us = u32_at(bytes, HEADER_LEN).ok_or(DecodeError::TooShort { len })?;
                Ok(Self::Keepalive { header, sent_at_us })
            }
            PacketKind::Join => exact(JOIN_LEN).map(|()| Self::Join(header)),
            PacketKind::Accept => exact(ACCEPT_LEN).map(|()| Self::Accept(header)),
            PacketKind::Close => {
                exact(CLOSE_LEN)?;
                let raw = bytes.get(HEADER_LEN).copied().unwrap_or_default();
                let reason =
                    CloseReason::from_raw(raw).ok_or(DecodeError::UnknownCloseReason { raw })?;
                Ok(Self::Close { header, reason })
            }
        }
    }

    fn decode_data(header: Header, raw_class: u8, bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let class = TrafficClass::from_raw(raw_class)
            .ok_or(DecodeError::UnknownClass { raw: raw_class })?;
        let too_short = DecodeError::TooShort { len: bytes.len() };
        let sequence = u32_at(bytes, HEADER_LEN).ok_or(too_short)?;
        let sent_at_us = u32_at(bytes, HEADER_LEN + 4).ok_or(too_short)?;
        let payload = bytes.get(DATA_HEADER_LEN..).ok_or(too_short)?;
        Ok(Self::Data(Data {
            header,
            class,
            sequence,
            sent_at_us,
            payload,
        }))
    }

    fn decode_ack(header: Header, bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let too_short = DecodeError::TooShort { len: bytes.len() };
        let word = |index: usize| u32_at(bytes, HEADER_LEN + 4 * index).ok_or(too_short);
        let timing = LinkTiming {
            largest_sequence: word(0)?,
            echoed_sent_at_us: word(1)?,
            ack_delay_us: word(2)?,
            received_bytes: word(3)?,
        };
        let ranges = bytes
            .get(ACK_HEADER_LEN..)
            .ok_or(too_short)
            .and_then(|rest| SequenceRanges::parse(rest).ok_or(DecodeError::MalformedAck))?;
        Ok(Self::Ack(Ack {
            header,
            timing,
            ranges,
        }))
    }

    /// Its kind.
    #[must_use]
    pub const fn kind(&self) -> PacketKind {
        match self {
            Self::Data(_) => PacketKind::Data,
            Self::Ack(_) => PacketKind::Ack,
            Self::Keepalive { .. } => PacketKind::Keepalive,
            Self::Join(_) => PacketKind::Join,
            Self::Accept(_) => PacketKind::Accept,
            Self::Close { .. } => PacketKind::Close,
        }
    }

    /// Its session and link.
    #[must_use]
    pub const fn header(&self) -> Header {
        match self {
            Self::Data(data) => data.header,
            Self::Ack(ack) => ack.header,
            Self::Keepalive { header, .. }
            | Self::Join(header)
            | Self::Accept(header)
            | Self::Close { header, .. } => *header,
        }
    }

    /// Length of its encoding, in bytes.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        match self {
            Self::Data(data) => DATA_HEADER_LEN + data.payload.len(),
            Self::Ack(ack) => ACK_HEADER_LEN + ack.ranges.bytes().len(),
            Self::Keepalive { .. } => KEEPALIVE_LEN,
            Self::Join(_) => JOIN_LEN,
            Self::Accept(_) => ACCEPT_LEN,
            Self::Close { .. } => CLOSE_LEN,
        }
    }

    /// Writes the packet at the start of `out` and returns its length.
    ///
    /// # Errors
    /// `out` too small, or a payload too long for one datagram.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let len = self.encoded_len();
        if len > MAX_DATAGRAM_LEN {
            return Err(EncodeError::TooLong { len });
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        match self {
            Self::Data(data) => {
                let rest = write_header(self.kind(), data.header, data.class.raw(), out);
                let (sequence, rest) = rest.split_at_mut(4);
                let (sent_at, payload) = rest.split_at_mut(4);
                sequence.copy_from_slice(&data.sequence.to_be_bytes());
                sent_at.copy_from_slice(&data.sent_at_us.to_be_bytes());
                payload.copy_from_slice(data.payload);
            }
            Self::Ack(ack) => {
                write_ack_header(ack.header, ack.timing, out).copy_from_slice(ack.ranges.bytes());
            }
            Self::Keepalive { header, sent_at_us } => {
                write_header(self.kind(), *header, 0, out)
                    .copy_from_slice(&sent_at_us.to_be_bytes());
            }
            Self::Join(header) | Self::Accept(header) => {
                write_header(self.kind(), *header, 0, out);
            }
            Self::Close { header, reason } => {
                write_header(self.kind(), *header, 0, out).copy_from_slice(&[reason.raw()]);
            }
        }
        Ok(len)
    }
}

/// Writes the 12-byte header into `out` (at least [`HEADER_LEN`] long) and returns what follows.
pub(crate) fn write_header(
    kind: PacketKind,
    header: Header,
    class: u8,
    out: &mut [u8],
) -> &mut [u8] {
    let (fixed, rest) = out.split_at_mut(HEADER_LEN);
    let (start, session) = fixed.split_at_mut(SESSION_OFFSET);
    start.copy_from_slice(&[MAGIC, kind.raw(), header.link_id, class]);
    session.copy_from_slice(&header.session_id.to_be_bytes());
    rest
}
