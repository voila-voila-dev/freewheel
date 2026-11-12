//! Keeping the requested bitrate inside its bounds, once per control step.

use crate::Benchmark;
use bitrate_control::Bounds;
use std::hint::black_box;

/// `bitrate_clamp`: one bound check per step.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "bitrate_clamp",
    run,
};

fn run(steps: u64) -> u64 {
    let bounds = Bounds {
        min_bps: 500_000,
        max_bps: 6_000_000,
    };
    let mut checksum = 0u64;
    for step in 0..steps {
        let requested = u32::try_from(black_box(step).wrapping_mul(7_919) % 9_000_000).unwrap_or(0);
        checksum = checksum.wrapping_add(u64::from(bounds.clamp(requested)));
    }
    checksum
}
