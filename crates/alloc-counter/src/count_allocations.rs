//! Measuring how many allocations a closure makes.

use crate::counting_allocator::ALLOCATIONS;

/// Runs `work` and returns its result with the number of allocations it made on this thread.
/// Only meaningful when [`crate::CountingAllocator`] is the global allocator.
pub fn count_allocations<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let before = ALLOCATIONS.with(std::cell::Cell::get);
    let result = work();
    let after = ALLOCATIONS.with(std::cell::Cell::get);
    (result, after - before)
}
