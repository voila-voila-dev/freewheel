//! A name for a link, shared with the I/O layer that holds its socket.

/// Names one link for the lifetime of the sender; never reused after the link is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LinkHandle(u64);

impl LinkHandle {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The number behind it, for logs.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
