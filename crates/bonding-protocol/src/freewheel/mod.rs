//! freewheel wire format v0, as specified in `docs/technical/protocol/freewheel.md`
//! ("Wire format v0").
//!
//! [`Packet::decode`] reads any datagram, [`Packet::encode`] writes it back byte for byte;
//! [`Ack::encode`] builds an acknowledgement from typed ranges. Nothing allocates.

mod ack;
mod close_reason;
mod data;
mod decode_error;
mod encode_error;
mod header;
mod lengths;
mod link_timing;
mod packet;
mod packet_kind;
mod read;
mod sequence_range;
mod traffic_class;

pub use ack::{Ack, SequenceRanges};
pub use close_reason::CloseReason;
pub use data::Data;
pub use decode_error::DecodeError;
pub use encode_error::EncodeError;
pub use header::{Header, LinkId, SessionId};
pub use lengths::{
    ACCEPT_LEN, ACK_HEADER_LEN, CLOSE_LEN, DATA_HEADER_LEN, HEADER_LEN, JOIN_LEN, KEEPALIVE_LEN,
    MAGIC, MAX_ACK_RANGES, MAX_DATA_PAYLOAD_LEN, MAX_DATAGRAM_LEN, MAX_RECORDING_CHUNK_LEN,
    RECORDING_CHUNK_HEADER_LEN, SEQUENCE_RANGE_LEN,
};
pub use link_timing::LinkTiming;
pub use packet::{Packet, is_freewheel};
pub use packet_kind::PacketKind;
pub use sequence_range::SequenceRange;
pub use traffic_class::TrafficClass;
