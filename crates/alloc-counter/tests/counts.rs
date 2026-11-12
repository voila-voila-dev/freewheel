//! The counter sees allocations, and only those of the measured closure.

use alloc_counter::{CountingAllocator, count_allocations};
use std::hint::black_box;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn counts_each_allocation() {
    let (_, count) = count_allocations(|| black_box(vec![0u8; 64]));
    assert_eq!(count, 1);
}

#[test]
fn counts_nothing_for_stack_work() {
    let (_, count) = count_allocations(|| {
        black_box([0u8; 64])
            .iter()
            .map(|&b| u64::from(b))
            .sum::<u64>()
    });
    assert_eq!(count, 0);
}
