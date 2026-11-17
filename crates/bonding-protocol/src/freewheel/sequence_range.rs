//! A run of consecutive sequences received.

/// Sequences `first` to `last`, both included, all received. Wraps around `u32::MAX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SequenceRange {
    /// First sequence of the run.
    pub first: u32,
    /// Last sequence of the run.
    pub last: u32,
}
