//! Decoding each kind of packet, with the length rules upstream applies.

mod support;

use bonding_protocol::srtla::*;
use support::{Random, encode, srt_control_with_words, srt_packet};

const RANGE_START_BIT: u32 = 1 << 31;

/// A loss list word starting a range at `first`.
const fn range_start(first: u32) -> u32 {
    RANGE_START_BIT | first
}

fn type_only(kind: PacketType) -> [u8; 2] {
    kind.raw().to_be_bytes()
}

#[test]
fn short_registration_answers_decode_to_their_kind() {
    for (kind, expected) in [
        (PacketType::Reg3, Packet::Reg3),
        (PacketType::RegErr, Packet::RegErr),
        (PacketType::RegNgp, Packet::RegNgp),
        (PacketType::RegNak, Packet::RegNak),
    ] {
        assert_eq!(Packet::decode(&type_only(kind)), Ok(expected));
        assert_eq!(expected.packet_type(), Some(kind));
    }
}

#[test]
fn reg1_and_reg2_carry_the_group_id() {
    let mut random = Random::new(1);
    let id = random.group_id();
    for (kind, expected) in [
        (PacketType::Reg1, Packet::Reg1(&id)),
        (PacketType::Reg2, Packet::Reg2(&id)),
    ] {
        let mut bytes = type_only(kind).to_vec();
        bytes.extend_from_slice(&id);
        assert_eq!(Packet::decode(&bytes), Ok(expected));
    }
}

#[test]
fn registration_of_the_wrong_length_is_srt_when_long_enough() {
    let mut random = Random::new(2);
    let too_long = srt_packet(&mut random, PacketType::Reg1.raw(), REG1_LEN + 1);
    let Ok(Packet::SrtControl(control)) = Packet::decode(&too_long) else {
        unreachable!("a 259-byte REG1 is forwarded to SRT upstream");
    };
    assert_eq!(control.raw_type(), PacketType::Reg1.raw());
    assert_eq!(Packet::SrtControl(control).packet_type(), None);

    let reg3_too_long = [0x92, 0x02, 0x00];
    assert_eq!(
        Packet::decode(&reg3_too_long),
        Err(DecodeError::TooShort { len: 3 })
    );
}

#[test]
fn keepalives_keep_whatever_follows_the_type() {
    assert_eq!(
        Packet::decode(&[0x90, 0x00]),
        Ok(Packet::Keepalive(Keepalive::default()))
    );
    let with_payload = [0x90, 0x00, 1, 2, 3];
    assert_eq!(
        Packet::decode(&with_payload),
        Ok(Packet::Keepalive(Keepalive {
            payload: &[1, 2, 3]
        }))
    );
}

#[test]
fn srtla_acks_list_sequence_numbers() {
    let numbers: Vec<SequenceNumber> = (100..110).filter_map(SequenceNumber::new).collect();
    let mut out = [0u8; 64];
    let Ok(len) = SrtlaAck::encode(&numbers, &mut out) else {
        unreachable!("10 numbers fit");
    };
    assert_eq!(len, 44, "upstream sends 44-byte SRTLA ACKs");
    assert_eq!(out[..4], [0x91, 0x00, 0x00, 0x00]);
    let Ok(Packet::SrtlaAck(ack)) = Packet::decode(&out[..len]) else {
        unreachable!("decodes as an SRTLA ACK");
    };
    assert_eq!(ack.len(), 10);
    assert!(ack.sequence_numbers().eq(numbers.iter().copied()));
}

#[test]
fn malformed_srtla_acks_are_rejected() {
    for bytes in [
        &[0x91, 0x00][..],
        &[0x91, 0x00, 0x00, 0x01],
        &[0x91, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
        &[0x91, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x01],
    ] {
        assert_eq!(Packet::decode(bytes), Err(DecodeError::MalformedSrtlaAck));
    }
}

#[test]
fn srtla_acks_are_limited_to_one_datagram() {
    let Some(one) = SequenceNumber::new(1) else {
        unreachable!("1 fits in 31 bits")
    };
    let numbers = vec![one; MAX_SRTLA_ACK_SEQUENCE_NUMBERS + 1];
    let mut out = vec![0u8; 2 * MAX_DATAGRAM_LEN];
    assert!(matches!(
        SrtlaAck::encode(&numbers, &mut out),
        Err(EncodeError::TooLong { .. })
    ));
    assert!(SrtlaAck::encode(&numbers[1..], &mut out).is_ok());
}

#[test]
fn data_packets_need_a_full_srt_header() {
    let mut random = Random::new(3);
    let data = srt_packet(&mut random, 0x0000, SRT_MIN_LEN);
    let Ok(Packet::SrtData(decoded)) = Packet::decode(&data) else {
        unreachable!("16 bytes with the top bit clear is SRT data");
    };
    assert_eq!(decoded.bytes(), data.as_slice());
    assert_eq!(
        Packet::decode(&data[..15]),
        Err(DecodeError::TooShort { len: 15 })
    );
}

#[test]
fn srt_acks_expose_the_last_acknowledged_number() {
    let mut random = Random::new(4);
    let bytes = srt_control_with_words(&mut random, PacketType::SrtAck, &[1234]);
    let Ok(Packet::SrtAck(ack)) = Packet::decode(&bytes) else {
        unreachable!("20-byte SRT ACK");
    };
    assert_eq!(ack.last_acknowledged().get(), 1234);

    let light = &bytes[..SRT_MIN_LEN];
    assert!(matches!(Packet::decode(light), Ok(Packet::SrtControl(_))));
}

#[test]
fn srt_naks_expose_single_losses_and_ranges() {
    let mut random = Random::new(5);
    let bytes = srt_control_with_words(
        &mut random,
        PacketType::SrtNak,
        &[7, range_start(10), 12, 20],
    );
    let Ok(Packet::SrtNak(nak)) = Packet::decode(&bytes) else {
        unreachable!("well-formed NAK");
    };
    let ranges: Vec<(u32, u32)> = nak
        .loss_ranges()
        .map(|range| (range.first.get(), range.last.get()))
        .collect();
    assert_eq!(ranges, [(7, 7), (10, 12), (20, 20)]);
}

#[test]
fn malformed_srt_naks_are_still_forwarded() {
    let mut random = Random::new(6);
    let unclosed = srt_control_with_words(&mut random, PacketType::SrtNak, &[range_start(10)]);
    let mut ragged = srt_control_with_words(&mut random, PacketType::SrtNak, &[7]);
    ragged.pop();
    let bad_end = srt_control_with_words(
        &mut random,
        PacketType::SrtNak,
        &[range_start(10), range_start(12)],
    );
    for bytes in [unclosed, ragged, bad_end] {
        let Ok(packet @ Packet::SrtControl(_)) = Packet::decode(&bytes) else {
            unreachable!("an unreadable NAK is still SRT traffic");
        };
        assert_eq!(packet.srt_bytes(), Some(bytes.as_slice()));
    }
}

#[test]
fn other_srt_control_packets_are_kept_whole() {
    let mut random = Random::new(7);
    for kind in [
        PacketType::SrtHandshake,
        PacketType::SrtKeepalive,
        PacketType::SrtShutdown,
        PacketType::SrtAckAck,
    ] {
        let bytes = srt_packet(&mut random, kind.raw(), 64);
        let Ok(packet @ Packet::SrtControl(_)) = Packet::decode(&bytes) else {
            unreachable!("{kind:?} is plain SRT control");
        };
        assert_eq!(packet.packet_type(), Some(kind));
        assert_eq!(encode(&packet), bytes);
    }
}

#[test]
fn only_srt_packets_are_forwarded_to_srt() {
    let mut random = Random::new(8);
    assert_eq!(Packet::Reg3.srt_bytes(), None);
    let data = srt_packet(&mut random, 0x0001, 1316);
    assert_eq!(
        Packet::decode(&data).map(|packet| packet.srt_bytes()),
        Ok(Some(data.as_slice()))
    );
}

#[test]
fn length_limits_hold_both_ways() {
    let too_long = vec![0u8; MAX_DATAGRAM_LEN + 1];
    assert_eq!(
        Packet::decode(&too_long),
        Err(DecodeError::TooLong {
            len: MAX_DATAGRAM_LEN + 1
        })
    );
    assert_eq!(Packet::decode(&[]), Err(DecodeError::TooShort { len: 0 }));
    assert_eq!(
        Packet::decode(&[0x90]),
        Err(DecodeError::TooShort { len: 1 })
    );

    let payload = [0u8; MAX_DATAGRAM_LEN];
    let oversized = Packet::Keepalive(Keepalive { payload: &payload });
    assert_eq!(
        oversized.encode(&mut [0u8; 2 * MAX_DATAGRAM_LEN]),
        Err(EncodeError::TooLong {
            len: MAX_DATAGRAM_LEN + 2
        })
    );
    assert_eq!(
        Packet::Reg3.encode(&mut [0u8; 1]),
        Err(EncodeError::BufferTooSmall { needed: 2 })
    );
}
