//! The 31-bit sequence number of an SRT data packet.

/// Largest SRT sequence number: they are 31 bits wide and wrap around after it.
pub const MAX_SEQUENCE_NUMBER: u32 = 0x7FFF_FFFF;

/// Sequence number of an SRT data packet. Always fits in 31 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SequenceNumber(u32);

impl SequenceNumber {
    /// Sequence number 0.
    pub const ZERO: Self = Self(0);

    /// `None` when the top bit is set: such a word is not a sequence number.
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value > MAX_SEQUENCE_NUMBER {
            None
        } else {
            Some(Self(value))
        }
    }

    /// The number itself.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
