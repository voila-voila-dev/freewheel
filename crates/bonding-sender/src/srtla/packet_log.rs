//! The last sequence numbers a link carried, to tell which link an ACK or a NAK is about.

use bonding_protocol::srtla::SequenceNumber;

use super::limits::PACKET_LOG_LEN;

/// Slots searched, newest first: every slot but the one the next number will overwrite.
const SEARCHED_LEN: usize = PACKET_LOG_LEN - 1;

/// A ring of the last [`PACKET_LOG_LEN`] data sequence numbers sent on a link.
///
/// Like upstream, a new log is full of zeros (not empty), and searches look at the
/// `PACKET_LOG_LEN - 1` newest slots only. Slots are addressed by age: 0 is the newest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PacketLog {
    slots: [Option<SequenceNumber>; PACKET_LOG_LEN],
    next: usize,
}

impl PacketLog {
    /// A log as upstream allocates it: every slot holds sequence number 0.
    pub(crate) const fn new() -> Self {
        Self {
            slots: [Some(SequenceNumber::ZERO); PACKET_LOG_LEN],
            next: 0,
        }
    }

    /// Empties every slot (the link failed).
    pub(crate) fn clear(&mut self) {
        self.slots.fill(None);
    }

    /// Logs a number sent on the link, overwriting the oldest.
    pub(crate) fn push(&mut self, number: SequenceNumber) {
        if let Some(slot) = self.slots.get_mut(self.next) {
            *slot = Some(number);
        }
        self.next = (self.next + 1) % PACKET_LOG_LEN;
    }

    fn index_of_age(&self, age: usize) -> usize {
        (self.next + PACKET_LOG_LEN - 1 - age) % PACKET_LOG_LEN
    }

    /// The number in the slot of that age (0 = newest), if any.
    pub(crate) fn at_age(&self, age: usize) -> Option<SequenceNumber> {
        self.slots.get(self.index_of_age(age)).copied().flatten()
    }

    /// Empties the slot of that age.
    pub(crate) fn clear_age(&mut self, age: usize) {
        let index = self.index_of_age(age);
        if let Some(slot) = self.slots.get_mut(index) {
            *slot = None;
        }
    }

    /// Searched ages, newest first.
    pub(crate) fn ages() -> core::ops::Range<usize> {
        0..SEARCHED_LEN
    }

    /// `true` when a slot younger than `age` holds `number`.
    pub(crate) fn holds_younger_than(&self, number: SequenceNumber, age: usize) -> bool {
        (0..age.min(SEARCHED_LEN)).any(|younger| self.at_age(younger) == Some(number))
    }

    /// `true` when a searched slot holds `number`.
    pub(crate) fn holds(&self, number: SequenceNumber) -> bool {
        self.holds_younger_than(number, SEARCHED_LEN)
    }

    /// Empties the newest slot holding `number`; `false` when none does.
    pub(crate) fn take(&mut self, number: SequenceNumber) -> bool {
        let Some(age) = Self::ages().find(|age| self.at_age(*age) == Some(number)) else {
            return false;
        };
        self.clear_age(age);
        true
    }

    /// Empties the searched slots below `number` and counts those left.
    pub(crate) fn forget_below(&mut self, number: SequenceNumber) -> u32 {
        let mut left = 0;
        for age in Self::ages() {
            match self.at_age(age) {
                Some(logged) if logged < number => self.clear_age(age),
                Some(_) => left += 1,
                None => {}
            }
        }
        left
    }
}
