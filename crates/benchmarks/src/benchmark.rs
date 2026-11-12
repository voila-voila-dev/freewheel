//! What a benchmark is.

/// A named piece of hot-path work, run once per packet.
#[derive(Debug, Clone, Copy)]
pub struct Benchmark {
    /// Stable name, used in `bench/baseline.json`.
    pub name: &'static str,
    /// Runs the work for `packets` packets and returns a checksum.
    pub run: fn(packets: u64) -> u64,
}
