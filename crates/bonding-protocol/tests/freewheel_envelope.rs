//! Wire format v1 before decryption: exact layouts, the lengths each kind allows, the
//! framing of a v0 packet for sealing, and round trips.

mod support;

use bonding_protocol::freewheel::*;
use support::Random;
use support::freewheel::{encode, encode_envelope, framed, header};

const SESSION: u64 = 0x0102_0304_0506_0708;
const LINK: u8 = 0x2A;

fn test_header() -> Header {
    Header {
        session_id: SESSION,
        link_id: LINK,
    }
}

#[test]
fn lengths_follow_from_the_overhead() {
    assert_eq!(SEALED_OVERHEAD_LEN, 24);
    assert_eq!(SEALED_HEADER_LEN, 20);
    assert_eq!(MAX_SEALED_DATA_PAYLOAD_LEN, 1_456);
    assert_eq!(MAX_SEALED_RECORDING_CHUNK_LEN, 1_447);
    assert_eq!(HELLO_LEN, 116);
    assert_eq!(WELCOME_LEN, 60);
}

#[test]
fn a_hello_is_laid_out_exactly() {
    let message: Vec<u8> = (0..=u8::MAX).take(HELLO_MESSAGE_LEN).collect();
    let bytes = encode_envelope(&Envelope::Hello(HandshakeMessage {
        header: test_header(),
        message: &message,
    }));
    assert_eq!(bytes[..12], [0xF1, 0x07, 0x2A, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(bytes[12..], message[..]);
    assert_eq!(
        Envelope::decode(&bytes),
        Ok(Envelope::Hello(HandshakeMessage {
            header: test_header(),
            message: &message,
        }))
    );
}

#[test]
fn a_welcome_is_laid_out_exactly() {
    let message = [9u8; WELCOME_MESSAGE_LEN];
    let bytes = encode_envelope(&Envelope::Welcome(HandshakeMessage {
        header: test_header(),
        message: &message,
    }));
    assert_eq!(bytes[..4], [0xF1, 0x08, 0x2A, 0]);
    assert_eq!(bytes.len(), WELCOME_LEN);
    assert!(matches!(
        Envelope::decode(&bytes),
        Ok(Envelope::Welcome(HandshakeMessage { message: m, .. })) if m == message
    ));
}

#[test]
fn a_sealed_packet_keeps_the_header_and_adds_the_nonce_and_tag() {
    let payload = [0xAB; 100];
    let plain = encode(&Packet::Data(Data {
        header: test_header(),
        class: TrafficClass::Background,
        sequence: 7,
        sent_at_us: 9,
        payload: &payload,
    }));
    let sealed = framed(&plain, 0x1122_3344_5566_7788);
    assert_eq!(sealed.len(), plain.len() + SEALED_OVERHEAD_LEN);
    assert_eq!(sealed[0], SEALED_MAGIC);
    assert_eq!(sealed[1..12], plain[1..12]);
    assert_eq!(
        sealed[12..20],
        [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]
    );
    assert_eq!(sealed[20..sealed.len() - TAG_LEN], plain[12..]);
    assert_eq!(sealed[sealed.len() - TAG_LEN..], [0; TAG_LEN]);
    let Ok(Envelope::Sealed(read)) = Envelope::decode(&sealed) else {
        unreachable!("a framed packet decodes")
    };
    assert_eq!(read.header, test_header());
    assert_eq!(read.kind, PacketKind::Data);
    assert_eq!(read.class, Some(TrafficClass::Background));
    assert_eq!(read.nonce, 0x1122_3344_5566_7788);
    assert_eq!(read.plain_len(), plain.len());
    assert_eq!(read.plain_header()[..], plain[..12]);
}

#[test]
fn the_plain_header_and_the_clear_bytes_give_back_the_v0_packet() {
    let mut random = Random::new(3);
    for plain in v0_packets(&mut random) {
        let sealed = framed(&plain, random.next_u64());
        let Ok(Envelope::Sealed(read)) = Envelope::decode(&sealed) else {
            unreachable!("a framed packet decodes: {plain:?}")
        };
        let mut rebuilt = read.plain_header().to_vec();
        rebuilt.extend_from_slice(&read.ciphertext[..read.ciphertext.len() - TAG_LEN]);
        assert_eq!(rebuilt, plain);
    }
}

#[test]
fn framing_refuses_what_is_not_v0_or_too_long() {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    assert_eq!(
        frame_sealed(&[MAGIC; 11], 0, &mut out),
        Err(EncodeError::NotPlain)
    );
    assert_eq!(
        frame_sealed(&[SEALED_MAGIC; 12], 0, &mut out),
        Err(EncodeError::NotPlain)
    );
    let payload = [0u8; MAX_DATA_PAYLOAD_LEN];
    let full = encode(&Packet::Data(Data {
        header: test_header(),
        class: TrafficClass::Live,
        sequence: 0,
        sent_at_us: 0,
        payload: &payload,
    }));
    assert_eq!(
        frame_sealed(&full, 0, &mut out),
        Err(EncodeError::TooLong { len: 1_524 })
    );
    let fits = encode(&Packet::Data(Data {
        header: test_header(),
        class: TrafficClass::Live,
        sequence: 0,
        sent_at_us: 0,
        payload: &payload[..MAX_SEALED_DATA_PAYLOAD_LEN],
    }));
    assert_eq!(frame_sealed(&fits, 0, &mut out), Ok(MAX_DATAGRAM_LEN));
    assert_eq!(
        frame_sealed(&fits, 0, &mut out[..100]),
        Err(EncodeError::BufferTooSmall {
            needed: MAX_DATAGRAM_LEN
        })
    );
}

#[test]
fn each_kind_has_the_sealed_lengths_of_its_v0_lengths() {
    let start = |kind: u8, class: u8, len: usize| {
        let mut bytes = vec![0u8; len];
        bytes[..4].copy_from_slice(&[SEALED_MAGIC, kind, LINK, class]);
        bytes
    };
    let ok = |bytes: &[u8]| Envelope::decode(bytes).is_ok();
    for (kind, v0_len) in [(0x03, 16), (0x04, 12), (0x05, 12), (0x06, 13)] {
        assert!(ok(&start(kind, 0, v0_len + 24)), "kind {kind}");
        assert!(!ok(&start(kind, 0, v0_len + 23)), "kind {kind}");
        assert!(!ok(&start(kind, 0, v0_len + 25)), "kind {kind}");
    }
    assert!(!ok(&start(0x01, 0x01, 43)));
    assert!(ok(&start(0x01, 0x01, 44)));
    assert!(ok(&start(0x01, 0x03, MAX_DATAGRAM_LEN)));
    assert!(!ok(&start(0x01, 0x03, MAX_DATAGRAM_LEN + 1)));
    assert!(!ok(&start(0x02, 0, 51)));
    assert!(ok(&start(0x02, 0, 52)));
    assert!(!ok(&start(0x02, 0, 56)));
    assert!(ok(&start(0x02, 0, 60)));
    assert!(ok(&start(HELLO_KIND, 0, HELLO_LEN)));
    assert!(!ok(&start(HELLO_KIND, 0, HELLO_LEN + 1)));
    assert!(ok(&start(WELCOME_KIND, 0, WELCOME_LEN)));
    assert!(!ok(&start(WELCOME_KIND, 0, WELCOME_LEN - 1)));
}

#[test]
fn what_v1_does_not_define_is_refused() {
    let with = |kind: u8, class: u8| {
        let mut bytes = vec![0u8; 44];
        bytes[..4].copy_from_slice(&[SEALED_MAGIC, kind, LINK, class]);
        Envelope::decode(&bytes).map(|_| ())
    };
    assert_eq!(
        with(0x09, 0),
        Err(EnvelopeDecodeError::UnknownKind { raw: 0x09 })
    );
    assert_eq!(
        with(0x00, 0),
        Err(EnvelopeDecodeError::UnknownKind { raw: 0x00 })
    );
    assert_eq!(
        with(0x01, 0x04),
        Err(EnvelopeDecodeError::UnknownClass { raw: 0x04 })
    );
    assert_eq!(with(0x02, 0x01), Err(EnvelopeDecodeError::ReservedNotZero));
    assert_eq!(
        Envelope::decode(&[MAGIC; 44]),
        Err(EnvelopeDecodeError::NotSealed)
    );
    assert_eq!(
        Envelope::decode(&[]),
        Err(EnvelopeDecodeError::TooShort { len: 0 })
    );
    assert_eq!(
        Envelope::decode(&[SEALED_MAGIC; 5]),
        Err(EnvelopeDecodeError::TooShort { len: 5 })
    );
}

#[test]
fn a_v0_packet_is_not_a_v1_one_and_back() {
    let mut random = Random::new(4);
    for plain in v0_packets(&mut random) {
        assert_eq!(
            Envelope::decode(&plain),
            Err(EnvelopeDecodeError::NotSealed)
        );
        let sealed = framed(&plain, 1);
        assert_eq!(Packet::decode(&sealed), Err(DecodeError::NotFreewheel));
        assert!(!is_freewheel(&sealed) && is_sealed(&sealed));
    }
}

#[test]
fn random_envelopes_round_trip() {
    let mut random = Random::new(5);
    for _ in 0..10_000 {
        let mut message = vec![0u8; HELLO_MESSAGE_LEN];
        random.fill(&mut message);
        let header = header(&mut random);
        for envelope in [
            Envelope::Hello(HandshakeMessage {
                header,
                message: &message,
            }),
            Envelope::Welcome(HandshakeMessage {
                header,
                message: &message[..WELCOME_MESSAGE_LEN],
            }),
        ] {
            assert_eq!(Envelope::decode(&encode_envelope(&envelope)), Ok(envelope));
        }
    }
    for plain in v0_packets(&mut random) {
        let sealed = framed(&plain, random.next_u64());
        let Ok(envelope) = Envelope::decode(&sealed) else {
            unreachable!("framed")
        };
        assert_eq!(encode_envelope(&envelope), sealed);
    }
}

/// One v0 packet of each kind and class, with random fields.
fn v0_packets(random: &mut Random) -> Vec<Vec<u8>> {
    let mut payload = vec![0u8; 1_316];
    random.fill(&mut payload);
    let header = header(random);
    let mut packets = Vec::new();
    for class in [
        TrafficClass::Live,
        TrafficClass::Reliable,
        TrafficClass::Background,
    ] {
        packets.push(encode(&Packet::Data(Data {
            header,
            class,
            sequence: random.next_u32(),
            sent_at_us: random.next_u32(),
            payload: &payload,
        })));
    }
    let ranges = support::freewheel::ranges(random, 5);
    packets.push(support::freewheel::encode_ack(
        header,
        support::freewheel::timing(random),
        &ranges,
    ));
    packets.push(encode(&Packet::Keepalive {
        header,
        sent_at_us: random.next_u32(),
    }));
    packets.push(encode(&Packet::Join(header)));
    packets.push(encode(&Packet::Accept(header)));
    packets.push(encode(&Packet::Close {
        header,
        reason: CloseReason::Refused,
    }));
    packets
}
