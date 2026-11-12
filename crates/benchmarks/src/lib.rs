//! Hot-path benchmarks. Each benchmark processes `packets` packets and returns a checksum,
//! so the compiler cannot remove the work.

mod benchmark;
pub mod benchmarks;
mod sample_packets;

pub use benchmark::Benchmark;
pub use benchmarks::ALL;
pub use sample_packets::SamplePackets;
