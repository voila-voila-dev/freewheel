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
