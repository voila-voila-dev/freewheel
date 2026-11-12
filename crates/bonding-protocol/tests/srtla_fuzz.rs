//! Decoding never panics, whatever arrives, and whatever decodes encodes back to itself.
//!
//! No third-party fuzzer (`decisions.md`): exhaustive short inputs, every type at every
//! boundary length, then seeded random mutations of valid packets. Debug builds run fewer
//! mutations; `cargo test --release` runs the full count.

mod support;

use bonding_protocol::srtla::*;
use support::{Random, assert_canonical, encode, srt_control_with_words, srt_packet};

/// Lengths where a rule changes: SRTLA answers, SRT header, SRT ACK, upstream SRTLA ACK,
/// REG1/REG2, the datagram limit.
const BOUNDARY_LENS: &[usize] = &[
    2, 3, 4, 5, 15, 16, 17, 19, 20, 21, 43, 44, 45, 257, 258, 259, 1_499, 1_500, 1_501,
];
const MUTATIONS: usize = if cfg!(debug_assertions) {
    100_000
} else {
    2_000_000
};

#[test]
fn every_input_of_up_to_two_bytes() {
    assert_canonical(&[]);
    for first in 0..=u8::MAX {
        assert_canonical(&[first]);
        for second in 0..=u8::MAX {
            assert_canonical(&[first, second]);
        }
    }
}

#[test]
fn every_type_at_every_boundary_length() {
    let mut random = Random::new(21);
    let mut bytes = vec![0u8; MAX_DATAGRAM_LEN + 1];
    for raw in 0..=u16::MAX {
        for &len in BOUNDARY_LENS {
            random.fill(&mut bytes[..len]);
            bytes[..2].copy_from_slice(&raw.to_be_bytes());
            assert_canonical(&bytes[..len]);
            bytes[2..len].fill(0);
            assert_canonical(&bytes[..len]);
        }
    }
}

#[test]
fn random_mutations_of_valid_packets() {
    let mut random = Random::new(22);
    let seeds = valid_packets(&mut random);
    for _ in 0..MUTATIONS {
        let mut bytes = seeds[random.below(seeds.len())].clone();
        for _ in 0..=random.below(4) {
            mutate(&mut random, &mut bytes);
        }
        assert_canonical(&bytes);
    }
}

/// One of each kind, as starting points for mutation.
fn valid_packets(random: &mut Random) -> Vec<Vec<u8>> {
    let id = random.group_id();
    let numbers: Vec<SequenceNumber> = (0..10).map(|_| random.sequence_number()).collect();
    let mut ack = [0u8; MAX_DATAGRAM_LEN];
    let Ok(ack_len) = SrtlaAck::encode(&numbers, &mut ack) else {
        unreachable!("10 numbers fit")
    };
    let mut packets = vec![
        encode(&Packet::Reg1(&id)),
        encode(&Packet::Reg2(&id)),
        encode(&Packet::Reg3),
        encode(&Packet::RegErr),
        encode(&Packet::RegNgp),
        encode(&Packet::Keepalive(Keepalive::default())),
        ack[..ack_len].to_vec(),
        srt_packet(random, 0x0000, 1_316),
        srt_packet(random, PacketType::SrtHandshake.raw(), 64),
        srt_control_with_words(random, PacketType::SrtAck, &[1, 2, 3, 4, 5, 6, 7]),
        srt_control_with_words(random, PacketType::SrtNak, &[5, (1 << 31) | 8, 12]),
    ];
    packets.push(srt_packet(random, PacketType::SrtShutdown.raw(), 16));
    packets
}

fn mutate(random: &mut Random, bytes: &mut Vec<u8>) {
    match random.below(6) {
        0 if !bytes.is_empty() => {
            let at = random.below(bytes.len());
            bytes[at] ^= 1 << random.below(8);
        }
        1 if !bytes.is_empty() => {
            let at = random.below(bytes.len());
            bytes[at] = random.byte();
        }
        2 => bytes.truncate(random.below(bytes.len() + 1)),
        3 => {
            for _ in 0..random.below(8) {
                bytes.push(random.byte());
            }
        }
        4 if bytes.len() >= 2 => {
            let raw: [u16; 9] = [
                0x8000, 0x8002, 0x8003, 0x9000, 0x9100, 0x9200, 0x9201, 0x9202, 0x9211,
            ];
            bytes[..2].copy_from_slice(&raw[random.below(raw.len())].to_be_bytes());
        }
        _ => {
            let len = random.below(MAX_DATAGRAM_LEN + 2);
            bytes.resize(len, 0);
        }
    }
}
