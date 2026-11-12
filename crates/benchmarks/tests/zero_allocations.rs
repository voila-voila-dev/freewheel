//! Hot paths must not allocate per packet: the allocation count must not grow with traffic.

use alloc_counter::{CountingAllocator, count_allocations};
use benchmarks::ALL;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn hot_paths_allocate_nothing_per_packet() {
    for benchmark in ALL {
        let (_, small) = count_allocations(|| (benchmark.run)(1_000));
        let (_, large) = count_allocations(|| (benchmark.run)(100_000));
        assert_eq!(
            small, large,
            "{}: {small} allocations for 1k packets, {large} for 100k: something allocates per packet",
            benchmark.name
        );
    }
}
