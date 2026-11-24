//! freewheel wire formats v0 and v1, as specified in `docs/technical/protocol/freewheel.md`
//! ("Wire format v0", "Wire format v1").
//!
//! [`Packet::decode`] reads any v0 datagram, [`Packet::encode`] writes it back byte for byte;
//! [`Ack::encode`] builds an acknowledgement from typed ranges; [`BackgroundPayload`] reads and
//! writes what a background data packet carries (padding, or a chunk of the recording).
//! [`Envelope`] reads and writes a v1 datagram before decryption (a sealed v0 packet, or a
//! handshake message), and [`frame_sealed`] lays out a v0 packet for sealing: the cryptography
//! itself is the sender's and the receiver's. Nothing allocates.

mod ack;
mod background_decode_error;
mod background_payload;
mod close_reason;
mod data;
mod decode_error;
mod encode_error;
mod envelope;
mod envelope_decode_error;
mod handshake_message;
mod header;
mod lengths;
mod link_timing;
mod packet;
mod packet_kind;
mod read;
mod recording_chunk;
mod sealed;
mod sequence_range;
mod traffic_class;

pub use ack::{Ack, SequenceRanges};
pub use background_decode_error::BackgroundDecodeError;
pub use background_payload::BackgroundPayload;
pub use close_reason::CloseReason;
pub use data::Data;
pub use decode_error::DecodeError;
pub use encode_error::EncodeError;
pub use envelope::{Envelope, is_sealed};
pub use envelope_decode_error::EnvelopeDecodeError;
pub use handshake_message::HandshakeMessage;
pub use header::{Header, LinkId, SessionId};
pub use lengths::{
    ACCEPT_LEN, ACK_HEADER_LEN, CLOSE_LEN, DATA_HEADER_LEN, HEADER_LEN, HELLO_KIND, HELLO_LEN,
    HELLO_MESSAGE_LEN, HELLO_PAYLOAD_LEN, JOIN_LEN, KEEPALIVE_LEN, MAGIC, MAX_ACK_RANGES,
    MAX_DATA_PAYLOAD_LEN, MAX_DATAGRAM_LEN, MAX_RECORDING_CHUNK_LEN, MAX_SEALED_DATA_PAYLOAD_LEN,
    MAX_SEALED_RECORDING_CHUNK_LEN, NONCE_LEN, PUBLIC_KEY_LEN, RECORDING_CHUNK_HEADER_LEN,
    SEALED_HEADER_LEN, SEALED_MAGIC, SEALED_OVERHEAD_LEN, SEQUENCE_RANGE_LEN, TAG_LEN,
    WELCOME_KIND, WELCOME_LEN, WELCOME_MESSAGE_LEN,
};
pub use link_timing::LinkTiming;
pub use packet::{Packet, is_freewheel};
pub use packet_kind::PacketKind;
pub use recording_chunk::RecordingChunk;
pub use sealed::{Sealed, frame_sealed};
pub use sequence_range::SequenceRange;
pub use traffic_class::TrafficClass;
