//! Property: for every freewheel packet, `decode(encode(packet)) == packet`, field by field.
//!
//! Packets are generated from random fields with a fixed seed, so a failure replays exactly.

mod support;

use bonding_protocol::freewheel::*;
use support::Random;
use support::freewheel::{CLASSES, REASONS, encode, encode_ack, header, ranges, timing};

const CASES: usize = 10_000;

fn round_trip(packet: Packet<'_>) {
    let bytes = encode(&packet);
    assert_eq!(Packet::decode(&bytes), Ok(packet));
}

#[test]
fn data_round_trips_in_every_class_with_any_payload() {
    let mut random = Random::new(31);
    let mut payload = vec![0u8; MAX_DATA_PAYLOAD_LEN];
    for case in 0..CASES {
        let len = match case {
            0 => 0,
            1 => MAX_DATA_PAYLOAD_LEN,
            _ => random.below(MAX_DATA_PAYLOAD_LEN + 1),
        };
        random.fill(&mut payload[..len]);
        round_trip(Packet::Data(Data {
            header: header(&mut random),
            class: CLASSES[case % CLASSES.len()],
            sequence: random.next_u32(),
            sent_at_us: random.next_u32(),
            payload: &payload[..len],
        }));
    }
}

#[test]
fn acks_round_trip_with_any_count() {
    let mut random = Random::new(32);
    for case in 0..CASES {
        let count = match case {
            0 => 0,
            1 => MAX_ACK_RANGES,
            _ => random.below(MAX_ACK_RANGES + 1),
        };
        let (header, timing, ranges) = (
            header(&mut random),
            timing(&mut random),
            ranges(&mut random, count),
        );
        let bytes = encode_ack(header, timing, &ranges);
        let Ok(packet @ Packet::Ack(ack)) = Packet::decode(&bytes) else {
            unreachable!("an encoded ACK decodes as one");
        };
        assert_eq!(
            (ack.header, ack.timing, ack.ranges.len()),
            (header, timing, count)
        );
        assert!(ack.ranges.iter().eq(ranges.iter().copied()));
        assert_eq!(encode(&packet), bytes);
    }
}

#[test]
fn session_packets_round_trip() {
    let mut random = Random::new(33);
    for case in 0..CASES {
        let header = header(&mut random);
        round_trip(Packet::Keepalive {
            header,
            sent_at_us: random.next_u32(),
        });
        round_trip(Packet::Join(header));
        round_trip(Packet::Accept(header));
        round_trip(Packet::Close {
            header,
            reason: REASONS[case % REASONS.len()],
        });
    }
}

#[test]
fn encoding_refuses_what_does_not_fit() {
    let header = Header::default();
    let too_many = vec![SequenceRange::default(); MAX_ACK_RANGES + 1];
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    assert_eq!(
        Ack::encode(header, LinkTiming::default(), &too_many, &mut out),
        Err(EncodeError::TooLong {
            len: ACK_HEADER_LEN + too_many.len() * SEQUENCE_RANGE_LEN
        })
    );
    let payload = [0u8; MAX_DATA_PAYLOAD_LEN + 1];
    let data = |payload| {
        Packet::Data(Data {
            header,
            class: TrafficClass::Live,
            sequence: 0,
            sent_at_us: 0,
            payload,
        })
    };
    assert_eq!(
        data(&payload).encode(&mut [0u8; 2 * MAX_DATAGRAM_LEN]),
        Err(EncodeError::TooLong {
            len: MAX_DATAGRAM_LEN + 1
        })
    );
    assert_eq!(
        Packet::Join(header).encode(&mut [0u8; JOIN_LEN - 1]),
        Err(EncodeError::BufferTooSmall { needed: JOIN_LEN })
    );
}
