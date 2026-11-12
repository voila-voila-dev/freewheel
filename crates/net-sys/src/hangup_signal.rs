//! Catching `SIGHUP`, the signal that asks a daemon to reload its configuration.

use std::ffi::c_int;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

/// `SIGHUP`: the same number on Linux and macOS.
const HANGUP: c_int = 1;
/// `SIG_ERR`: what `signal(2)` returns when it fails.
const SIGNAL_ERROR: usize = usize::MAX;

static HANGUP_RECEIVED: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn signal(number: c_int, handler: extern "C" fn(c_int)) -> usize;
}

extern "C" fn on_hangup(_: c_int) {
    // Async-signal-safe: a single atomic store.
    HANGUP_RECEIVED.store(true, Ordering::Relaxed);
}

/// From now on, `SIGHUP` no longer terminates the process: it sets [`hangup_flag`], which the
/// program reads (and clears) when it is ready to reload.
///
/// # Errors
/// What `signal(2)` reports.
pub fn catch_hangup() -> io::Result<()> {
    // SAFETY: `on_hangup` is a plain `extern "C"` function that lives for the whole program and
    // only stores to a static atomic, which is async-signal-safe.
    let previous = unsafe { signal(HANGUP, on_hangup) };
    if previous == SIGNAL_ERROR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Set by every `SIGHUP` once [`catch_hangup`] ran. Swap it to `false` to consume it.
#[must_use]
pub fn hangup_flag() -> &'static AtomicBool {
    &HANGUP_RECEIVED
}
