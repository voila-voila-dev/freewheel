//! The few C library calls std does not expose, behind safe wrappers.
//!
//! - `poll(2)`: waiting on several sockets at once, so one thread serves every link.
//! - `setsockopt(SO_SNDBUF)`: a larger send buffer per link, as upstream's sender asks.
//! - `signal(SIGHUP)`: reloading the sender's link list, as belaUI asks upstream's.
//!
//! The only crate of the bonding path allowed `unsafe` (`docs/technical/decisions.md`).

mod hangup_signal;
mod readiness_slot;
mod send_buffer;
mod wait_until_readable;

pub use hangup_signal::{catch_hangup, hangup_flag};
pub use readiness_slot::ReadinessSlot;
pub use send_buffer::set_send_buffer_bytes;
pub use wait_until_readable::wait_until_readable;
