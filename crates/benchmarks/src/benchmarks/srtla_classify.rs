//! Classifying every incoming packet: the first thing the receiver does per packet.

use crate::{Benchmark, SamplePackets};
use bonding_protocol::srtla::{classify, data_sequence_number};
use std::hint::black_box;

/// `srtla_classify`: type, length check and sequence number of each packet.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "srtla_classify",
    run,
};

fn run(packets: u64) -> u64 {
    let sample = SamplePackets::new();
    let mut checksum = 0u64;
    for index in 0..packets {
        let packet = black_box(sample.get(index));
        checksum = checksum.wrapping_add(classify(packet).map_or(0, |kind| kind as u64));
        checksum = checksum.wrapping_add(u64::from(data_sequence_number(packet).unwrap_or(0)));
    }
    checksum
}
