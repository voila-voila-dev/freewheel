//! Reading the recording chunks of background data packets, then writing them back: what the
//! server does per chunk of the recording upload, and the sender per chunk sent.

use crate::Benchmark;
use bonding_protocol::freewheel::{
    BackgroundPayload, Data, Header, MAX_DATA_PAYLOAD_LEN, MAX_DATAGRAM_LEN,
    MAX_RECORDING_CHUNK_LEN, Packet, RecordingChunk, TrafficClass,
};
use std::hint::black_box;

/// `recording_chunk_decode`: packet, then payload, decoded; the payload encoded back into a
/// reused buffer.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "recording_chunk_decode",
    run,
};

/// Chunks in the sample, consecutive in the recording.
const CHUNKS: u32 = 16;

fn run(packets: u64) -> u64 {
    let sample = sample_packets();
    let mut out = [0u8; MAX_DATA_PAYLOAD_LEN];
    let mut checksum = 0u64;
    for index in 0..packets {
        let slot = usize::try_from(index % sample.len() as u64).unwrap_or(0);
        let packet = black_box(sample.get(slot).map_or(&[][..], Vec::as_slice));
        let written = match Packet::decode(packet) {
            Ok(Packet::Data(data)) => BackgroundPayload::decode(data.payload)
                .ok()
                .and_then(|payload| payload.encode(&mut out).ok())
                .unwrap_or(0),
            _ => 0,
        };
        checksum = checksum.wrapping_add(written as u64);
    }
    checksum
}

/// Full chunks of the recording, and one padding packet.
fn sample_packets() -> Vec<Vec<u8>> {
    let header = Header {
        session_id: 0x5EED,
        link_id: 1,
    };
    let recording = [0xA5u8; MAX_RECORDING_CHUNK_LEN];
    let mut payload = [0u8; MAX_DATA_PAYLOAD_LEN];
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let mut packets = Vec::new();
    let mut payloads: Vec<BackgroundPayload<'_>> = (0..CHUNKS)
        .map(|index| {
            BackgroundPayload::Chunk(RecordingChunk {
                offset: u64::from(index) * MAX_RECORDING_CHUNK_LEN as u64,
                bytes: &recording,
            })
        })
        .collect();
    payloads.push(BackgroundPayload::Padding {
        zeros: MAX_DATA_PAYLOAD_LEN - 1,
    });
    for (sequence, background) in (0..).zip(&payloads) {
        let Ok(len) = background.encode(&mut payload) else {
            continue;
        };
        let data = Packet::Data(Data {
            header,
            class: TrafficClass::Background,
            sequence,
            sent_at_us: sequence * 250,
            payload: payload.get(..len).unwrap_or_default(),
        });
        if let Ok(len) = data.encode(&mut out) {
            packets.push(out[..len].to_vec());
        }
    }
    packets
}
