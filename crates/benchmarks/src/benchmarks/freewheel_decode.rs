//! Decoding every freewheel packet then encoding it back: what our receiver and sender will
//! do per packet.

use crate::Benchmark;
use bonding_protocol::freewheel::{
    Ack, Data, Header, LinkTiming, MAX_DATAGRAM_LEN, Packet, SequenceRange, TrafficClass,
};
use std::hint::black_box;

/// `freewheel_decode`: full decode, then encode into a reused buffer.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "freewheel_decode",
    run,
};

/// Live data packets in a stream, per ACK.
const DATA_PER_ACK: u32 = 16;
/// Ranges in a typical ACK: a few holes in the last window.
const RANGES_PER_ACK: usize = 4;
/// Payload of one live data packet: seven MPEG-TS packets, as SRT carries them.
const LIVE_PAYLOAD_LEN: usize = 1_316;

fn run(packets: u64) -> u64 {
    let sample = sample_packets();
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let mut checksum = 0u64;
    for index in 0..packets {
        let slot = usize::try_from(index % sample.len() as u64).unwrap_or(0);
        let packet = black_box(sample.get(slot).map_or(&[][..], Vec::as_slice));
        let written = Packet::decode(packet)
            .ok()
            .and_then(|decoded| decoded.encode(&mut out).ok())
            .unwrap_or(0);
        checksum = checksum.wrapping_add(written as u64);
    }
    checksum
}

/// Mostly live data, one ACK and one keepalive: the mix on a busy link.
fn sample_packets() -> Vec<Vec<u8>> {
    let header = Header {
        session_id: 0x5EED,
        link_id: 1,
    };
    let payload = [0u8; LIVE_PAYLOAD_LEN];
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let mut packets = Vec::new();
    for sequence in 0..DATA_PER_ACK {
        let data = Packet::Data(Data {
            header,
            class: TrafficClass::Live,
            sequence,
            sent_at_us: sequence * 700,
            payload: &payload,
        });
        if let Ok(len) = data.encode(&mut out) {
            packets.push(out[..len].to_vec());
        }
    }
    let ranges = [SequenceRange::default(); RANGES_PER_ACK];
    if let Ok(len) = Ack::encode(header, LinkTiming::default(), &ranges, &mut out) {
        packets.push(out[..len].to_vec());
    }
    let keepalive = Packet::Keepalive {
        header,
        sent_at_us: 0,
    };
    if let Ok(len) = keepalive.encode(&mut out) {
        packets.push(out[..len].to_vec());
    }
    packets
}
