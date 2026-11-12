//! Asking the kernel for a larger send buffer on a socket.

use std::ffi::{c_int, c_void};
use std::io;
use std::os::fd::{AsFd, AsRawFd};

#[cfg(target_os = "linux")]
const SOCKET_LEVEL: c_int = 1;
#[cfg(not(target_os = "linux"))]
const SOCKET_LEVEL: c_int = 0xFFFF;
#[cfg(target_os = "linux")]
const SEND_BUFFER_OPTION: c_int = 7;
#[cfg(not(target_os = "linux"))]
const SEND_BUFFER_OPTION: c_int = 0x1001;

/// C's `socklen_t`: 32 bits unsigned on Linux and macOS.
type OptionLen = u32;

unsafe extern "C" {
    fn setsockopt(
        descriptor: c_int,
        level: c_int,
        option: c_int,
        value: *const c_void,
        value_len: OptionLen,
    ) -> c_int;
}

/// Sets `SO_SNDBUF` on `socket`. The kernel may grant less than asked (Linux caps it at
/// `net.core.wmem_max`, then doubles it) without reporting an error.
///
/// # Errors
/// What `setsockopt(2)` reports, or a size that does not fit a C `int`.
pub fn set_send_buffer_bytes(socket: &impl AsFd, bytes: usize) -> io::Result<()> {
    let value = c_int::try_from(bytes).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let value_len = OptionLen::try_from(size_of::<c_int>()).unwrap_or(OptionLen::MAX);
    // SAFETY: the descriptor is borrowed from a live socket for the duration of the call;
    // `value` points to one `c_int` on our stack, which outlives the call, and `value_len` is
    // exactly its size. `setsockopt` only reads it and keeps no pointer.
    let result = unsafe {
        setsockopt(
            socket.as_fd().as_raw_fd(),
            SOCKET_LEVEL,
            SEND_BUFFER_OPTION,
            (&raw const value).cast::<c_void>(),
            value_len,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
