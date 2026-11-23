//! Decoding a background payload never panics, whatever arrives, and whatever decodes encodes
//! back to itself. Same method as `freewheel_fuzz.rs`.

mod support;

use bonding_protocol::freewheel::*;
use support::Random;
use support::freewheel::{assert_background_canonical, encode_background};

/// Lengths where a rule changes: no kind, kind alone, a chunk's header and one byte, the
/// payload limit.
const BOUNDARY_LENS: &[usize] = &[0, 1, 2, 8, 9, 10, 11, 1_470, 1_471, 1_479, 1_480, 1_481];
const MUTATIONS: usize = if cfg!(debug_assertions) {
    100_000
} else {
    2_000_000
};

#[test]
fn every_input_of_up_to_two_bytes() {
    assert_background_canonical(&[]);
    for first in 0..=u8::MAX {
        assert_background_canonical(&[first]);
        for second in 0..=u8::MAX {
            assert_background_canonical(&[first, second]);
        }
    }
}

#[test]
fn every_kind_at_every_boundary_length() {
    let mut random = Random::new(62);
    let mut bytes = vec![0u8; MAX_DATA_PAYLOAD_LEN + 1];
    for kind in 0..=u8::MAX {
        for &len in BOUNDARY_LENS {
            random.fill(&mut bytes[..len]);
            if let Some(first) = bytes.get_mut(0).filter(|_| len > 0) {
                *first = kind;
            }
            assert_background_canonical(&bytes[..len]);
            if len > 1 {
                bytes[1..len].fill(0);
            }
            assert_background_canonical(&bytes[..len]);
        }
    }
}

#[test]
fn random_mutations_of_valid_payloads() {
    let mut random = Random::new(63);
    let mut recording = vec![0u8; MAX_RECORDING_CHUNK_LEN];
    random.fill(&mut recording);
    let seeds = [
        encode_background(&BackgroundPayload::Padding {
            zeros: MAX_DATA_PAYLOAD_LEN - 1,
        }),
        encode_background(&BackgroundPayload::Padding { zeros: 3 }),
        encode_background(&BackgroundPayload::Chunk(RecordingChunk {
            offset: random.next_u64(),
            bytes: &recording,
        })),
        encode_background(&BackgroundPayload::Chunk(RecordingChunk {
            offset: random.next_u64(),
            bytes: &recording[..1],
        })),
    ];
    for _ in 0..MUTATIONS {
        let mut bytes = seeds[random.below(seeds.len())].clone();
        for _ in 0..=random.below(4) {
            match random.below(5) {
                0 if !bytes.is_empty() => {
                    let at = random.below(bytes.len());
                    bytes[at] ^= 1 << random.below(8);
                }
                1 if !bytes.is_empty() => bytes[0] = u8::try_from(random.below(4)).unwrap_or(0),
                2 => bytes.truncate(random.below(bytes.len() + 1)),
                3 => bytes.push(random.byte()),
                _ => bytes.resize(random.below(MAX_DATA_PAYLOAD_LEN + 2), 0),
            }
        }
        assert_background_canonical(&bytes);
    }
}
