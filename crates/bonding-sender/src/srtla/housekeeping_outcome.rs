//! What housekeeping tells the program.

/// Whether the sender should keep going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HousekeepingOutcome {
    /// Keep going.
    Running,
    /// Every link failed for [`GIVE_UP_AFTER_MS`](super::GIVE_UP_AFTER_MS) and nothing is left
    /// to try: upstream exits with status 1 here.
    GaveUp,
}
