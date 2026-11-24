//! Every benchmark, one per file.

mod bitrate_clamp;
mod freewheel_decode;
mod freewheel_encrypt;
mod receiver_forward;
mod recording_chunk_decode;
mod sender_forward;
mod srtla_classify;
mod srtla_decode;

use crate::Benchmark;

/// All benchmarks, in report order.
pub const ALL: &[Benchmark] = &[
    srtla_classify::BENCHMARK,
    srtla_decode::BENCHMARK,
    freewheel_decode::BENCHMARK,
    freewheel_encrypt::CHACHA20POLY1305,
    freewheel_encrypt::AES256GCM,
    recording_chunk_decode::BENCHMARK,
    receiver_forward::BENCHMARK,
    sender_forward::BENCHMARK,
    bitrate_clamp::BENCHMARK,
];
