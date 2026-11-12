//! A name for a group, shared with the I/O layer that holds its SRT socket.

/// Names one group for the lifetime of the receiver; never reused after the group is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupHandle(u64);

impl GroupHandle {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The number behind it, for logs.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}
