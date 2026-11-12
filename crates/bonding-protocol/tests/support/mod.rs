//! Shared helpers for wire format tests: a seeded generator and packet builders.

#![allow(dead_code)]

use bonding_protocol::srtla::{
    GroupId, ID_LEN, MAX_DATAGRAM_LEN, MAX_SEQUENCE_NUMBER, Packet, PacketType, SRT_MIN_LEN,
    SequenceNumber,
};

/// Small deterministic generator: same seed, same inputs, on every machine.
pub struct Random(u64);

impl Random {
    /// A generator; seed 0 is replaced, since xorshift would stay at 0.
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        u32::try_from(self.next_u64() >> 32).unwrap_or_default()
    }

    /// Uniform in `[0, bound)`; `bound` must not be 0.
    pub fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next_u64() % bound as u64).unwrap_or_default()
    }

    /// One random byte.
    pub fn byte(&mut self) -> u8 {
        self.next_u64().to_le_bytes()[0]
    }

    /// Fills `bytes` with random bytes.
    pub fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            *byte = self.byte();
        }
    }

    /// A random 31-bit sequence number.
    pub fn sequence_number(&mut self) -> SequenceNumber {
        let Some(number) = SequenceNumber::new(self.next_u32() & MAX_SEQUENCE_NUMBER) else {
            unreachable!("masked to 31 bits")
        };
        number
    }

    /// A random group id.
    pub fn group_id(&mut self) -> GroupId {
        let mut id = [0u8; ID_LEN];
        self.fill(&mut id);
        id
    }
}

/// An SRT packet of `len` bytes (at least 16) whose first 16 bits are `first_word_high`.
pub fn srt_packet(random: &mut Random, first_word_high: u16, len: usize) -> Vec<u8> {
    assert!((SRT_MIN_LEN..=MAX_DATAGRAM_LEN).contains(&len));
    let mut bytes = vec![0u8; len];
    random.fill(&mut bytes);
    bytes[..2].copy_from_slice(&first_word_high.to_be_bytes());
    bytes
}

/// An SRT control packet of type `kind` followed by `words` (big-endian) from byte 16.
pub fn srt_control_with_words(random: &mut Random, kind: PacketType, words: &[u32]) -> Vec<u8> {
    let mut bytes = srt_packet(random, kind.raw(), SRT_MIN_LEN);
    for word in words {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes
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
