//! Decoding a freewheel v1 datagram never panics, whatever arrives, and whatever decodes
//! encodes back to itself: the v0 method (`freewheel_fuzz.rs`) on [`Envelope`].

mod support;

use bonding_protocol::freewheel::*;
use support::Random;
use support::freewheel::{
    CLASSES, REASONS, assert_envelope_canonical, encode, encode_ack, encode_envelope, framed,
    header, ranges, timing,
};

/// Lengths where a rule changes: the header, each fixed kind sealed (v0 length + 24), the
/// smallest sealed data and ACK, an ACK range, the handshakes, the datagram limit.
const BOUNDARY_LENS: &[usize] = &[
    0, 1, 2, 3, 4, 11, 12, 13, 19, 20, 21, 35, 36, 37, 38, 39, 40, 41, 43, 44, 45, 51, 52, 53, 59,
    60, 61, 115, 116, 117, 1_499, 1_500, 1_501,
];
const MUTATIONS: usize = if cfg!(debug_assertions) {
    100_000
} else {
    2_000_000
};

#[test]
fn every_input_of_up_to_two_bytes() {
    assert_envelope_canonical(&[]);
    for first in 0..=u8::MAX {
        assert_envelope_canonical(&[first]);
        for second in 0..=u8::MAX {
            assert_envelope_canonical(&[first, second]);
        }
    }
}

#[test]
fn every_kind_and_class_at_every_boundary_length() {
    let mut random = Random::new(51);
    let mut bytes = vec![0u8; MAX_DATAGRAM_LEN + 1];
    for kind in 0..=u8::MAX {
        for class in 0..=u8::MAX {
            for &len in BOUNDARY_LENS {
                random.fill(&mut bytes[..len]);
                let start = [SEALED_MAGIC, kind, random.byte(), class];
                let prefix = len.min(start.len());
                bytes[..prefix].copy_from_slice(&start[..prefix]);
                assert_envelope_canonical(&bytes[..len]);
                bytes[prefix..len].fill(0);
                assert_envelope_canonical(&bytes[..len]);
            }
        }
    }
}

#[test]
fn random_mutations_of_valid_envelopes() {
    let mut random = Random::new(52);
    let seeds = valid_envelopes(&mut random);
    for _ in 0..MUTATIONS {
        let mut bytes = seeds[random.below(seeds.len())].clone();
        for _ in 0..=random.below(4) {
            mutate(&mut random, &mut bytes);
        }
        assert_envelope_canonical(&bytes);
    }
}

/// One of each kind, sealed, and both handshake messages, as starting points for mutation.
fn valid_envelopes(random: &mut Random) -> Vec<Vec<u8>> {
    let mut payload = vec![0u8; 1_316];
    random.fill(&mut payload);
    let header = header(random);
    let mut plain: Vec<Vec<u8>> = CLASSES
        .iter()
        .map(|&class| {
            encode(&Packet::Data(Data {
                header,
                class,
                sequence: random.next_u32(),
                sent_at_us: random.next_u32(),
                payload: &payload,
            }))
        })
        .collect();
    let ack_ranges = ranges(random, 10);
    plain.push(encode_ack(header, timing(random), &ack_ranges));
    plain.push(encode(&Packet::Keepalive {
        header,
        sent_at_us: random.next_u32(),
    }));
    plain.push(encode(&Packet::Join(header)));
    plain.push(encode(&Packet::Accept(header)));
    for reason in REASONS {
        plain.push(encode(&Packet::Close { header, reason }));
    }
    let mut envelopes: Vec<Vec<u8>> = plain
        .iter()
        .map(|packet| framed(packet, random.next_u64()))
        .collect();
    let mut message = vec![0u8; HELLO_MESSAGE_LEN];
    random.fill(&mut message);
    envelopes.push(encode_envelope(&Envelope::Hello(HandshakeMessage {
        header,
        message: &message,
    })));
    envelopes.push(encode_envelope(&Envelope::Welcome(HandshakeMessage {
        header,
        message: &message[..WELCOME_MESSAGE_LEN],
    })));
    envelopes
}

fn mutate(random: &mut Random, bytes: &mut Vec<u8>) {
    match random.below(6) {
        0 if !bytes.is_empty() => {
            let at = random.below(bytes.len());
            bytes[at] ^= 1 << random.below(8);
        }
        1 if !bytes.is_empty() => {
            let at = random.below(bytes.len().min(SEALED_HEADER_LEN + 1));
            bytes[at] = random.byte();
        }
        2 => bytes.truncate(random.below(bytes.len() + 1)),
        3 => {
            for _ in 0..random.below(8) {
                bytes.push(random.byte());
            }
        }
        4 if bytes.len() >= 2 => bytes[1] = u8::try_from(random.below(10)).unwrap_or_default(),
        _ => {
            let len = random.below(MAX_DATAGRAM_LEN + 2);
            bytes.resize(len, 0);
        }
    }
}
