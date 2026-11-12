//! SRTLA v2 wire format (compatibility mode), as specified in `docs/technical/protocol/srtla.md`.
//!
//! [`Packet::decode`] reads any datagram, [`Packet::encode`] writes it back byte for byte;
//! [`SrtlaAck::encode`] builds an ACK from sequence numbers. Nothing allocates.

mod classify;
mod decode_error;
mod encode_error;
mod keepalive;
mod lengths;
mod loss_range;
mod packet;
mod packet_type;
mod read;
mod sequence_number;
mod srt_ack;
mod srt_control;
mod srt_data;
mod srt_nak;
mod srtla_ack;

pub use classify::classify;
pub use decode_error::DecodeError;
pub use encode_error::EncodeError;
pub use keepalive::Keepalive;
pub use lengths::{
    HALF_ID_LEN, ID_LEN, MAX_DATAGRAM_LEN, MAX_SRTLA_ACK_SEQUENCE_NUMBERS, REG_ANSWER_LEN,
    REG1_LEN, REG2_LEN, REG3_LEN, SEQUENCE_NUMBER_LEN, SRT_ACK_MIN_LEN, SRT_MIN_LEN,
    SRTLA_ACK_HEADER_LEN, TYPE_LEN,
};
pub use loss_range::LossRange;
pub use packet::{GroupId, Packet};
pub use packet_type::PacketType;
pub use read::{data_sequence_number, raw_type};
pub use sequence_number::{MAX_SEQUENCE_NUMBER, SequenceNumber};
pub use srt_ack::SrtAck;
pub use srt_control::SrtControl;
pub use srt_data::SrtData;
pub use srt_nak::{LossRanges, SrtNak};
pub use srtla_ack::SrtlaAck;
