//! Property: for every packet, `decode(encode(packet)) == packet`, field by field.
//!
//! Packets are generated from random fields with a fixed seed, so a failure replays exactly.

mod support;

use bonding_protocol::srtla::*;
use support::{Random, encode, srt_control_with_words, srt_packet};

const CASES: usize = 10_000;
const RANGE_START_BIT: u32 = 1 << 31;

fn round_trip(packet: Packet<'_>) {
    let bytes = encode(&packet);
    assert_eq!(Packet::decode(&bytes), Ok(packet));
}

#[test]
fn registration_packets_round_trip() {
    let mut random = Random::new(11);
    for _ in 0..CASES {
        let id = random.group_id();
        round_trip(Packet::Reg1(&id));
        round_trip(Packet::Reg2(&id));
    }
    for packet in [Packet::Reg3, Packet::RegErr, Packet::RegNgp, Packet::RegNak] {
        round_trip(packet);
    }
}

#[test]
fn keepalives_round_trip_with_any_payload() {
    let mut random = Random::new(12);
    for _ in 0..CASES {
        let mut payload = vec![0u8; random.below(MAX_DATAGRAM_LEN - TYPE_LEN + 1)];
        random.fill(&mut payload);
        round_trip(Packet::Keepalive(Keepalive { payload: &payload }));
    }
}

#[test]
fn srtla_acks_round_trip_with_any_count() {
    let mut random = Random::new(13);
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    for _ in 0..CASES {
        let count = random.below(MAX_SRTLA_ACK_SEQUENCE_NUMBERS + 1);
        let numbers: Vec<SequenceNumber> = (0..count).map(|_| random.sequence_number()).collect();
        let Ok(len) = SrtlaAck::encode(&numbers, &mut out) else {
            unreachable!("{count} numbers fit in one datagram");
        };
        let Ok(packet @ Packet::SrtlaAck(ack)) = Packet::decode(&out[..len]) else {
            unreachable!("an encoded SRTLA ACK decodes as one");
        };
        assert_eq!(ack.len(), count);
        assert!(ack.sequence_numbers().eq(numbers.iter().copied()));
        assert_eq!(encode(&packet), out[..len]);
    }
}

#[test]
fn srt_data_round_trips_with_its_sequence_number() {
    let mut random = Random::new(14);
    for _ in 0..CASES {
        let sequence_number = random.sequence_number();
        let len = SRT_MIN_LEN + random.below(MAX_DATAGRAM_LEN - SRT_MIN_LEN + 1);
        let mut bytes = srt_packet(&mut random, 0, len);
        bytes[..4].copy_from_slice(&sequence_number.get().to_be_bytes());
        let Ok(packet @ Packet::SrtData(data)) = Packet::decode(&bytes) else {
            unreachable!("top bit clear and 16+ bytes is SRT data");
        };
        assert_eq!(data.sequence_number(), sequence_number);
        assert_eq!(encode(&packet), bytes);
    }
}

#[test]
fn srt_acks_round_trip_with_their_last_acknowledged_number() {
    let mut random = Random::new(15);
    for _ in 0..CASES {
        let last = random.sequence_number();
        let extra_words = random.below(8);
        let mut words = vec![last.get()];
        words.extend((0..extra_words).map(|_| random.next_u32()));
        let bytes = srt_control_with_words(&mut random, PacketType::SrtAck, &words);
        let Ok(packet @ Packet::SrtAck(ack)) = Packet::decode(&bytes) else {
            unreachable!("an SRT ACK with a 31-bit last number");
        };
        assert_eq!(ack.last_acknowledged(), last);
        assert_eq!(encode(&packet), bytes);
    }
}

#[test]
fn srt_naks_round_trip_with_their_loss_list() {
    let mut random = Random::new(16);
    for _ in 0..CASES {
        let mut expected = Vec::new();
        let mut words = Vec::new();
        for _ in 0..random.below(40) {
            let first = random.sequence_number();
            if random.below(2) == 0 {
                words.push(first.get());
                expected.push(LossRange { first, last: first });
            } else {
                let last = random.sequence_number();
                words.extend([first.get() | RANGE_START_BIT, last.get()]);
                expected.push(LossRange { first, last });
            }
        }
        let bytes = srt_control_with_words(&mut random, PacketType::SrtNak, &words);
        let Ok(packet @ Packet::SrtNak(nak)) = Packet::decode(&bytes) else {
            unreachable!("a well-formed NAK");
        };
        assert!(nak.loss_ranges().eq(expected.iter().copied()));
        assert_eq!(encode(&packet), bytes);
    }
}

#[test]
fn other_srt_control_packets_round_trip_unchanged() {
    let mut random = Random::new(17);
    for _ in 0..CASES {
        let raw = 0x8000 | u16::from_be_bytes([random.byte(), random.byte()]);
        let len = SRT_MIN_LEN + random.below(MAX_DATAGRAM_LEN - SRT_MIN_LEN + 1);
        let bytes = srt_packet(&mut random, raw, len);
        let Ok(packet) = Packet::decode(&bytes) else {
            // Only a malformed SRTLA ACK is refused at this length.
            assert_eq!(raw, PacketType::SrtlaAck.raw());
            continue;
        };
        assert_eq!(encode(&packet), bytes);
    }
}
