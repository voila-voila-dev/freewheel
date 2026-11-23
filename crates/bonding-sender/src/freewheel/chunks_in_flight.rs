//! The recording chunks sent and not resolved yet (delivered or lost), in the order they left.

/// One copy of a chunk on its way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ChunkInFlight {
    pub(crate) sequence: u32,
    pub(crate) link_id: u8,
    pub(crate) chunk: u64,
    /// Bytes of the recording it carries: the chunk size, or less for the recording's tail.
    pub(crate) len: u16,
    pub(crate) sent_at_us: u64,
    /// Delivered or lost already: waits to leave the front of the ring.
    pub(crate) resolved: bool,
}

/// A ring of chunk copies in sequence order (the order they were sent), allocated once.
/// Resolved copies leave once they reach the front.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunksInFlight {
    entries: Vec<ChunkInFlight>,
    oldest: usize,
    count: usize,
}

/// Sequences wrap: within half the space, a sequence "after" another is ahead of it.
const HALF_SEQUENCE_SPACE: u32 = u32::MAX / 2;

impl ChunksInFlight {
    /// An empty ring of `capacity` copies (at least one).
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: vec![ChunkInFlight::default(); capacity.max(1)],
            oldest: 0,
            count: 0,
        }
    }

    /// No room for another copy.
    pub(crate) const fn is_full(&self) -> bool {
        self.count == self.entries.len()
    }

    /// Copies held, resolved ones not at the front yet included.
    pub(crate) const fn len(&self) -> usize {
        self.count
    }

    /// Remembers a copy just sent (its sequence after every other's); `false` if full.
    pub(crate) fn push(&mut self, entry: ChunkInFlight) -> bool {
        if self.is_full() {
            return false;
        }
        let slot = (self.oldest + self.count) % self.entries.len();
        if let Some(stored) = self.entries.get_mut(slot) {
            *stored = entry;
            self.count += 1;
        }
        true
    }

    /// The copy at `position` from the oldest.
    pub(crate) fn get_mut(&mut self, position: usize) -> Option<&mut ChunkInFlight> {
        if position >= self.count {
            return None;
        }
        let slot = (self.oldest + position) % self.entries.len();
        self.entries.get_mut(slot)
    }

    /// The copy at `position` from the oldest.
    pub(crate) fn get(&self, position: usize) -> Option<&ChunkInFlight> {
        if position >= self.count {
            return None;
        }
        self.entries
            .get((self.oldest + position) % self.entries.len())
    }

    /// Positions of the copies sent with a sequence from `first` to `last` (wrapping, both
    /// included): a contiguous run, since sequences grow in sending order.
    pub(crate) fn positions_between(&self, first: u32, last: u32) -> core::ops::Range<usize> {
        let Some(front) = self.get(0).map(|entry| entry.sequence) else {
            return 0..0;
        };
        let (from, to) = (first.wrapping_sub(front), last.wrapping_sub(front));
        if to > HALF_SEQUENCE_SPACE || (from > to && from <= HALF_SEQUENCE_SPACE) {
            // Wholly before the oldest copy, or not a run.
            return 0..0;
        }
        let from = if from > HALF_SEQUENCE_SPACE { 0 } else { from };
        self.first_position_from(from)..self.first_position_from(to.saturating_add(1))
    }

    /// The first position whose sequence is at least `offset` after the oldest's.
    fn first_position_from(&self, offset: u32) -> usize {
        let Some(front) = self.get(0).map(|entry| entry.sequence) else {
            return 0;
        };
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let middle = low + (high - low) / 2;
            let ahead = self
                .get(middle)
                .map_or(u32::MAX, |entry| entry.sequence.wrapping_sub(front));
            if ahead < offset {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    /// Forgets the resolved copies at the front.
    pub(crate) fn drop_resolved_front(&mut self) {
        while self.get(0).is_some_and(|entry| entry.resolved) {
            self.oldest = (self.oldest + 1) % self.entries.len();
            self.count -= 1;
        }
    }
}
