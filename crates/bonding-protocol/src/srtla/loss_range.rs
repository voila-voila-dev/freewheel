//! One entry of an SRT NAK loss list.

use super::sequence_number::SequenceNumber;

/// Data packets reported lost, from `first` to `last` inclusive (equal for a single loss).
///
/// `last` may be below `first` when the range wraps around the sequence space; nothing here
/// reorders it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LossRange {
    /// First lost sequence number.
    pub first: SequenceNumber,
    /// Last lost sequence number, inclusive.
    pub last: SequenceNumber,
}
