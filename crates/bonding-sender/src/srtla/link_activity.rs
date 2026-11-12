//! When a link last heard from the receiver.

use super::limits::LINK_TIMEOUT_S;

/// When a link last heard from the receiver (`REG_NGP` and REG2 do not count).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkActivity {
    /// Never, or not since it was declared failed.
    Never,
    /// A send failed on it: failed at once, declared so at the next housekeeping.
    SendFailed,
    /// Its last packet from the receiver arrived during this second.
    ReceivedAt {
        /// Whole seconds since the sender started.
        second: u64,
    },
}

impl LinkActivity {
    /// `true` when the link must not carry data: silent for more than [`LINK_TIMEOUT_S`].
    pub(crate) const fn timed_out(self, now_s: u64) -> bool {
        match self {
            Self::Never | Self::SendFailed => true,
            Self::ReceivedAt { second } => second + LINK_TIMEOUT_S < now_s,
        }
    }
}
