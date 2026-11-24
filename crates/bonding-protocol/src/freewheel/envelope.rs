//! Every freewheel v1 datagram, before decryption: a sealed packet, a hello or a welcome.

use super::encode_error::EncodeError;
use super::envelope_decode_error::EnvelopeDecodeError;
use super::handshake_message::HandshakeMessage;
use super::header::Header;
use super::lengths::{
    ACK_HEADER_LEN, CLOSE_LEN, DATA_HEADER_LEN, HEADER_LEN, HELLO_KIND, HELLO_LEN, JOIN_LEN,
    KEEPALIVE_LEN, MAX_DATAGRAM_LEN, SEALED_HEADER_LEN, SEALED_MAGIC, SEQUENCE_RANGE_LEN, TAG_LEN,
    WELCOME_KIND, WELCOME_LEN,
};
use super::packet_kind::PacketKind;
use super::read::u64_at;
use super::sealed::Sealed;
use super::traffic_class::TrafficClass;

const KIND_OFFSET: usize = 1;
const LINK_OFFSET: usize = 2;
const CLASS_OFFSET: usize = 3;
const SESSION_OFFSET: usize = 4;

/// `true` when a datagram is a freewheel v1 packet (starts with [`SEALED_MAGIC`]).
#[must_use]
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.first() == Some(&SEALED_MAGIC)
}

/// One freewheel v1 datagram, borrowed from the buffer it was read from, before anything is
/// decrypted (`docs/technical/protocol/freewheel.md`, "Wire format v1").
///
/// As in v0, decoding then encoding gives back the same bytes, and anything v1 does not
/// define is refused: the lengths a sealed packet may have follow from its kind's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Envelope<'a> {
    /// A v0 packet, authenticated and encrypted.
    Sealed(Sealed<'a>),
    /// The box opens a session: the handshake's first message.
    Hello(HandshakeMessage<'a>),
    /// The server accepts it: the handshake's second message.
    Welcome(HandshakeMessage<'a>),
}

impl<'a> Envelope<'a> {
    /// Reads one datagram.
    ///
    /// # Errors
    /// See [`EnvelopeDecodeError`].
    pub fn decode(bytes: &'a [u8]) -> Result<Self, EnvelopeDecodeError> {
        let len = bytes.len();
        if len == 0 {
            return Err(EnvelopeDecodeError::TooShort { len });
        }
        if !is_sealed(bytes) {
            return Err(EnvelopeDecodeError::NotSealed);
        }
        if len > MAX_DATAGRAM_LEN {
            return Err(EnvelopeDecodeError::TooLong { len });
        }
        let (Some(&raw_kind), Some(&link_id), Some(&raw_class), Some(session_id)) = (
            bytes.get(KIND_OFFSET),
            bytes.get(LINK_OFFSET),
            bytes.get(CLASS_OFFSET),
            u64_at(bytes, SESSION_OFFSET),
        ) else {
            return Err(EnvelopeDecodeError::TooShort { len });
        };
        let header = Header {
            session_id,
            link_id,
        };
        let is_data = raw_kind == PacketKind::Data.raw();
        if !is_data && raw_class != 0 {
            if PacketKind::from_raw(raw_kind).is_none()
                && raw_kind != HELLO_KIND
                && raw_kind != WELCOME_KIND
            {
                return Err(EnvelopeDecodeError::UnknownKind { raw: raw_kind });
            }
            return Err(EnvelopeDecodeError::ReservedNotZero);
        }
        let wrong_length = EnvelopeDecodeError::WrongLength {
            kind: raw_kind,
            len,
        };
        let handshake = |expected: usize| {
            if len == expected {
                bytes
                    .get(HEADER_LEN..)
                    .map(|message| HandshakeMessage { header, message })
                    .ok_or(EnvelopeDecodeError::TooShort { len })
            } else {
                Err(wrong_length)
            }
        };
        match raw_kind {
            HELLO_KIND => return handshake(HELLO_LEN).map(Self::Hello),
            WELCOME_KIND => return handshake(WELCOME_LEN).map(Self::Welcome),
            _ => {}
        }
        let kind = PacketKind::from_raw(raw_kind)
            .ok_or(EnvelopeDecodeError::UnknownKind { raw: raw_kind })?;
        let class = if is_data {
            Some(
                TrafficClass::from_raw(raw_class)
                    .ok_or(EnvelopeDecodeError::UnknownClass { raw: raw_class })?,
            )
        } else {
            None
        };
        // The v0 packet inside, from its sealed length: what its kind allows.
        let plain_len = len
            .checked_sub(SEALED_HEADER_LEN - HEADER_LEN + TAG_LEN)
            .filter(|plain_len| *plain_len >= HEADER_LEN)
            .ok_or(EnvelopeDecodeError::TooShort { len })?;
        let allowed = match kind {
            PacketKind::Data => plain_len >= DATA_HEADER_LEN,
            PacketKind::Ack => {
                plain_len >= ACK_HEADER_LEN
                    && (plain_len - ACK_HEADER_LEN).is_multiple_of(SEQUENCE_RANGE_LEN)
            }
            PacketKind::Keepalive => plain_len == KEEPALIVE_LEN,
            PacketKind::Join | PacketKind::Accept => plain_len == JOIN_LEN,
            PacketKind::Close => plain_len == CLOSE_LEN,
        };
        if !allowed {
            return Err(wrong_length);
        }
        let (Some(nonce), Some(ciphertext)) =
            (u64_at(bytes, HEADER_LEN), bytes.get(SEALED_HEADER_LEN..))
        else {
            return Err(EnvelopeDecodeError::TooShort { len });
        };
        Ok(Self::Sealed(Sealed {
            header,
            kind,
            class,
            nonce,
            ciphertext,
        }))
    }

    /// Its session and link.
    #[must_use]
    pub const fn header(&self) -> Header {
        match self {
            Self::Sealed(sealed) => sealed.header,
            Self::Hello(message) | Self::Welcome(message) => message.header,
        }
    }

    /// Length of its encoding, in bytes.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        match self {
            Self::Sealed(sealed) => sealed.encoded_len(),
            Self::Hello(message) | Self::Welcome(message) => HEADER_LEN + message.message.len(),
        }
    }

    /// Writes it at the start of `out` and returns its length.
    ///
    /// # Errors
    /// `out` too small, or longer than one datagram.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let (kind, message) = match self {
            Self::Sealed(sealed) => return sealed.encode(out),
            Self::Hello(message) => (HELLO_KIND, message),
            Self::Welcome(message) => (WELCOME_KIND, message),
        };
        let len = self.encoded_len();
        if len > MAX_DATAGRAM_LEN {
            return Err(EncodeError::TooLong { len });
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        let (fixed, rest) = out.split_at_mut(HEADER_LEN);
        let (start, session) = fixed.split_at_mut(SESSION_OFFSET);
        start.copy_from_slice(&[SEALED_MAGIC, kind, message.header.link_id, 0]);
        session.copy_from_slice(&message.header.session_id.to_be_bytes());
        rest.copy_from_slice(message.message);
        Ok(len)
    }
}
