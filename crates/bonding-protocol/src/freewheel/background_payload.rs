//! What a background data packet carries: padding, or a chunk of the recording.

use super::background_decode_error::BackgroundDecodeError;
use super::encode_error::EncodeError;
use super::lengths::{MAX_DATA_PAYLOAD_LEN, MAX_RECORDING_CHUNK_LEN, RECORDING_CHUNK_HEADER_LEN};
use super::read::u64_at;
use super::recording_chunk::RecordingChunk;

/// First byte of padding.
const PADDING_KIND: u8 = 0x00;
/// First byte of a recording chunk.
const CHUNK_KIND: u8 = 0x01;
const KIND_LEN: usize = 1;
const ZERO: u8 = 0;

/// The payload of a data packet of the background class
/// (`docs/technical/protocol/freewheel.md`, "Background payload").
///
/// Decoding then encoding gives back the same bytes, and anything v0 does not define is
/// refused: an unknown kind, padding that is not all zero, a chunk without bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundPayload<'a> {
    /// Capacity probing's filler: the kind byte, then `zeros` zero bytes. Nothing to keep.
    Padding {
        /// Zero bytes after the kind byte.
        zeros: usize,
    },
    /// Bytes of the recording.
    Chunk(RecordingChunk<'a>),
}

impl<'a> BackgroundPayload<'a> {
    /// Reads the payload of a background data packet.
    ///
    /// # Errors
    /// See [`BackgroundDecodeError`].
    pub fn decode(payload: &'a [u8]) -> Result<Self, BackgroundDecodeError> {
        let len = payload.len();
        if len > MAX_DATA_PAYLOAD_LEN {
            return Err(BackgroundDecodeError::TooLong { len });
        }
        let Some((&kind, rest)) = payload.split_first() else {
            return Err(BackgroundDecodeError::Empty);
        };
        match kind {
            PADDING_KIND => {
                if rest.iter().any(|byte| *byte != ZERO) {
                    return Err(BackgroundDecodeError::PaddingNotZero);
                }
                Ok(Self::Padding { zeros: rest.len() })
            }
            CHUNK_KIND => {
                let (Some(offset), Some(bytes)) = (
                    u64_at(payload, KIND_LEN),
                    payload.get(RECORDING_CHUNK_HEADER_LEN..),
                ) else {
                    return Err(BackgroundDecodeError::ChunkTooShort { len });
                };
                if bytes.is_empty() {
                    return Err(BackgroundDecodeError::EmptyChunk);
                }
                Ok(Self::Chunk(RecordingChunk { offset, bytes }))
            }
            raw => Err(BackgroundDecodeError::UnknownKind { raw }),
        }
    }

    /// Length of its encoding, in bytes.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        match self {
            Self::Padding { zeros } => KIND_LEN + *zeros,
            Self::Chunk(chunk) => RECORDING_CHUNK_HEADER_LEN + chunk.bytes.len(),
        }
    }

    /// Writes the payload at the start of `out` and returns its length.
    ///
    /// # Errors
    /// `out` too small, more than one data packet carries, or a chunk without bytes
    /// ([`EncodeError::EmptyChunk`]).
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let len = self.encoded_len();
        if len > MAX_DATA_PAYLOAD_LEN {
            return Err(EncodeError::TooLong { len });
        }
        if let Self::Chunk(chunk) = self
            && chunk.bytes.is_empty()
        {
            return Err(EncodeError::EmptyChunk);
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        let (kind, rest) = out.split_at_mut(KIND_LEN);
        match self {
            Self::Padding { .. } => {
                kind.copy_from_slice(&[PADDING_KIND]);
                rest.fill(ZERO);
            }
            Self::Chunk(chunk) => {
                kind.copy_from_slice(&[CHUNK_KIND]);
                let (offset, bytes) = rest.split_at_mut(RECORDING_CHUNK_HEADER_LEN - KIND_LEN);
                offset.copy_from_slice(&chunk.offset.to_be_bytes());
                bytes.copy_from_slice(chunk.bytes);
            }
        }
        Ok(len)
    }
}

// A full chunk fills a data packet exactly.
const _: () = assert!(RECORDING_CHUNK_HEADER_LEN + MAX_RECORDING_CHUNK_LEN == MAX_DATA_PAYLOAD_LEN);
