//! One socket watched by [`wait_until_readable`](crate::wait_until_readable).

use std::ffi::{c_int, c_short};
use std::os::fd::{AsFd, AsRawFd};

/// `POLLIN`: data to read. Same value on Linux and macOS.
const READABLE: c_short = 0x001;
/// `POLLERR`, `POLLHUP`, `POLLNVAL`: a pending error, a hang-up, a closed descriptor.
/// Each makes the next read return at once, so they count as readable.
const ERROR_OR_CLOSED: c_short = 0x008 | 0x010 | 0x020;

/// A socket to watch, laid out as C's `struct pollfd`.
///
/// It holds the raw descriptor, not a borrow: if the socket is closed before the wait, `poll`
/// reports it as invalid (it never touches memory through it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ReadinessSlot {
    descriptor: c_int,
    requested: c_short,
    returned: c_short,
}

impl ReadinessSlot {
    /// Watches `socket` for something to read.
    #[must_use]
    pub fn new(socket: &impl AsFd) -> Self {
        Self {
            descriptor: socket.as_fd().as_raw_fd(),
            requested: READABLE,
            returned: 0,
        }
    }

    /// `true` when the next read on the socket will not block: a datagram or an error is waiting.
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.returned & (READABLE | ERROR_OR_CLOSED) != 0
    }
}
