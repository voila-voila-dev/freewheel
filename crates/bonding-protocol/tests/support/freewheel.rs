//! Helpers for freewheel wire format tests.

use super::Random;
use bonding_protocol::freewheel::{
    Ack, CloseReason, Header, LinkTiming, MAX_DATAGRAM_LEN, Packet, SequenceRange, TrafficClass,
};

/// Every traffic class.
pub const CLASSES: [TrafficClass; 3] = [
    TrafficClass::Live,
    TrafficClass::Reliable,
    TrafficClass::Background,
];

/// Every close reason.
pub const REASONS: [CloseReason; 3] = [
    CloseReason::Done,
    CloseReason::Refused,
    CloseReason::TimedOut,
];

/// A random session and link.
pub fn header(random: &mut Random) -> Header {
    Header {
        session_id: random.next_u64(),
        link_id: random.byte(),
    }
}

/// Random timing fields.
pub fn timing(random: &mut Random) -> LinkTiming {
    LinkTiming {
        largest_sequence: random.next_u32(),
        echoed_sent_at_us: random.next_u32(),
        ack_delay_us: random.next_u32(),
        received_bytes: random.next_u32(),
    }
}

/// `count` random ranges.
pub fn ranges(random: &mut Random, count: usize) -> Vec<SequenceRange> {
    (0..count)
        .map(|_| SequenceRange {
            first: random.next_u32(),
            last: random.next_u32(),
        })
        .collect()
}

/// Encodes an ACK into a fresh buffer of exactly its length.
pub fn encode_ack(header: Header, timing: LinkTiming, ranges: &[SequenceRange]) -> Vec<u8> {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let Ok(len) = Ack::encode(header, timing, ranges, &mut out) else {
        unreachable!("{} ranges fit in one datagram", ranges.len())
    };
    out[..len].to_vec()
}

/// Encodes `packet` into a fresh buffer of exactly its length.
pub fn encode(packet: &Packet<'_>) -> Vec<u8> {
    let mut out = vec![0u8; packet.encoded_len()];
    let Ok(len) = packet.encode(&mut out) else {
        unreachable!("buffer sized from encoded_len")
    };
    assert_eq!(len, out.len());
    out
}

/// Asserts the core property on one datagram: if it decodes, it encodes back to itself.
pub fn assert_canonical(bytes: &[u8]) {
    if let Ok(packet) = Packet::decode(bytes) {
        assert_eq!(packet.encoded_len(), bytes.len(), "{packet:?}");
        assert_eq!(encode(&packet), bytes, "{packet:?}");
    }
}

/// Encodes a background payload into a fresh buffer of exactly its length.
pub fn encode_background(payload: &bonding_protocol::freewheel::BackgroundPayload<'_>) -> Vec<u8> {
    let mut out = vec![0u8; payload.encoded_len()];
    let Ok(len) = payload.encode(&mut out) else {
        unreachable!("buffer sized from encoded_len, {payload:?}")
    };
    assert_eq!(len, out.len());
    out
}

/// Asserts the core property on one background payload: if it decodes, it encodes back to
/// itself.
pub fn assert_background_canonical(bytes: &[u8]) {
    if let Ok(payload) = bonding_protocol::freewheel::BackgroundPayload::decode(bytes) {
        assert_eq!(payload.encoded_len(), bytes.len(), "{payload:?}");
        assert_eq!(encode_background(&payload), bytes, "{payload:?}");
    }
}

/// Encodes a v1 envelope into a fresh buffer of exactly its length.
pub fn encode_envelope(envelope: &bonding_protocol::freewheel::Envelope<'_>) -> Vec<u8> {
    let mut out = vec![0u8; envelope.encoded_len()];
    let Ok(len) = envelope.encode(&mut out) else {
        unreachable!("buffer sized from encoded_len")
    };
    assert_eq!(len, out.len());
    out
}

/// Asserts the core property on one v1 datagram: if it decodes, it encodes back to itself.
pub fn assert_envelope_canonical(bytes: &[u8]) {
    if let Ok(envelope) = bonding_protocol::freewheel::Envelope::decode(bytes) {
        assert_eq!(envelope.encoded_len(), bytes.len(), "{envelope:?}");
        assert_eq!(encode_envelope(&envelope), bytes, "{envelope:?}");
    }
}

/// The sealed layout of an encoded v0 packet, its encrypted part left in clear and its tag
/// zero: what [`frame_sealed`](bonding_protocol::freewheel::frame_sealed) writes.
pub fn framed(plain: &[u8], nonce: u64) -> Vec<u8> {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let Ok(len) = bonding_protocol::freewheel::frame_sealed(plain, nonce, &mut out) else {
        unreachable!("{} bytes fit sealed", plain.len())
    };
    out[..len].to_vec()
}
