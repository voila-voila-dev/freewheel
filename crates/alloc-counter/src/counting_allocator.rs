//! A global allocator that forwards to the system allocator and counts calls per thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// Allocations made by this thread. `const` init: reading it never allocates.
    pub(crate) static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// Counts every allocation and reallocation, per thread, then defers to [`System`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CountingAllocator;

fn record() {
    ALLOCATIONS.with(|count| count.set(count.get() + 1));
}

// SAFETY: every method forwards to `System` with the caller's arguments unchanged;
// counting only touches a thread-local `Cell`, which never allocates.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: same contract as `GlobalAlloc::alloc`, upheld by the caller.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: same contract as `GlobalAlloc::alloc_zeroed`, upheld by the caller.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        // SAFETY: same contract as `GlobalAlloc::realloc`, upheld by the caller.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as `GlobalAlloc::dealloc`, upheld by the caller.
        unsafe { System.dealloc(ptr, layout) }
    }
}
