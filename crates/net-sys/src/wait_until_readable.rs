//! Waiting until at least one of several sockets has something to read.

use std::ffi::c_int;
use std::io;
use std::time::Duration;

use crate::ReadinessSlot;

/// C's `nfds_t`: `unsigned long` on Linux, `unsigned int` on macOS and the BSDs.
#[cfg(target_os = "linux")]
type SlotCount = std::ffi::c_ulong;
#[cfg(not(target_os = "linux"))]
type SlotCount = std::ffi::c_uint;

unsafe extern "C" {
    fn poll(slots: *mut ReadinessSlot, count: SlotCount, timeout_ms: c_int) -> c_int;
}

/// Blocks until a socket in `slots` is ready or `timeout` has passed, and returns how many are
/// ready (0 on timeout, or when a signal interrupted the wait). Then check each slot with
/// [`ReadinessSlot::is_ready`].
///
/// # Errors
/// What `poll(2)` reports, e.g. too many slots for the process.
pub fn wait_until_readable(slots: &mut [ReadinessSlot], timeout: Duration) -> io::Result<usize> {
    let timeout_ms = c_int::try_from(timeout.as_millis()).unwrap_or(c_int::MAX);
    let count = SlotCount::try_from(slots.len())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `slots` is an exclusively borrowed array of exactly `count` `ReadinessSlot`s, whose
    // `#[repr(C)]` layout is `struct pollfd` (int, short, short). `poll` only writes the
    // `revents` field of those entries and keeps no pointer after it returns.
    let ready = unsafe { poll(slots.as_mut_ptr(), count, timeout_ms) };
    if ready < 0 {
        let error = io::Error::last_os_error();
        return if error.kind() == io::ErrorKind::Interrupted {
            Ok(0)
        } else {
            Err(error)
        };
    }
    Ok(usize::try_from(ready).unwrap_or_default())
}
