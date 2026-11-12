//! Every benchmark, one per file.

mod bitrate_clamp;
mod receiver_forward;
mod sender_forward;
mod srtla_classify;
mod srtla_decode;

use crate::Benchmark;

/// All benchmarks, in report order.
pub const ALL: &[Benchmark] = &[
    srtla_classify::BENCHMARK,
    srtla_decode::BENCHMARK,
    receiver_forward::BENCHMARK,
    sender_forward::BENCHMARK,
    bitrate_clamp::BENCHMARK,
];
