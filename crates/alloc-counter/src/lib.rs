//! Counts heap allocations made by the current thread.
//!
//! Install it as the global allocator in a test or benchmark binary:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOCATOR: alloc_counter::CountingAllocator = alloc_counter::CountingAllocator;
//! ```
//!
//! then measure a closure with [`count_allocations`].

mod count_allocations;
mod counting_allocator;

pub use count_allocations::count_allocations;
pub use counting_allocator::CountingAllocator;
