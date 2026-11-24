//! Why a sealed packet was not opened.

/// Why a sealed packet was not opened: either way it is dropped, never answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenError {
    /// Its nonce was already opened, or is below the replay window.
    Replayed,
    /// It does not authenticate: altered, forged, or sealed with other keys.
    Forged,
    /// The output buffer is shorter than the packet inside.
    BufferTooSmall,
}
