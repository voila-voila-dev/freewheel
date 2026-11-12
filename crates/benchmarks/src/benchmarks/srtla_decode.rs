//! Decoding every packet then encoding it back: what the receiver and sender do per packet.

use crate::{Benchmark, SamplePackets};
use bonding_protocol::srtla::{MAX_DATAGRAM_LEN, Packet};
use std::hint::black_box;

/// `srtla_decode`: full decode, then encode into a reused buffer.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "srtla_decode",
    run,
};

fn run(packets: u64) -> u64 {
    let sample = SamplePackets::new();
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let mut checksum = 0u64;
    for index in 0..packets {
        let packet = black_box(sample.get(index));
        let written = Packet::decode(packet)
            .ok()
            .and_then(|decoded| decoded.encode(&mut out).ok())
            .unwrap_or(0);
        checksum = checksum.wrapping_add(written as u64);
    }
    checksum
}
