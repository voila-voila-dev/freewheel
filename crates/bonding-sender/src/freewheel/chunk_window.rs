//! Which chunks of the recording are delivered or in flight, over a window that slides with
//! the first chunk not delivered yet.

const BITS_PER_WORD: u64 = 64;

/// Two bits per chunk over `slots` chunks from `base`, the first chunk not delivered yet:
/// delivered, and in flight. Every chunk before `base` is delivered; none beyond the window is
/// tracked, so the upload sends nothing there. Allocated once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChunkWindow {
    delivered: Vec<u64>,
    in_flight: Vec<u64>,
    slots: u64,
    base: u64,
}

impl ChunkWindow {
    /// A window of `slots` chunks (at least one), from chunk 0.
    pub(crate) fn new(slots: u64) -> Self {
        let slots = slots.max(1);
        let words = usize::try_from(slots.div_ceil(BITS_PER_WORD)).unwrap_or(usize::MAX);
        Self {
            delivered: vec![0; words],
            in_flight: vec![0; words],
            slots,
            base: 0,
        }
    }

    /// The first chunk not delivered yet.
    pub(crate) const fn base(&self) -> u64 {
        self.base
    }

    /// The chunk just past the window: nothing from there on may be sent yet.
    pub(crate) const fn end(&self) -> u64 {
        self.base.saturating_add(self.slots)
    }

    /// `chunk` has been delivered.
    pub(crate) fn is_delivered(&self, chunk: u64) -> bool {
        chunk < self.base || (chunk < self.end() && bit(&self.delivered, self.slots, chunk))
    }

    /// A copy of `chunk` is in flight.
    pub(crate) fn is_in_flight(&self, chunk: u64) -> bool {
        (self.base..self.end()).contains(&chunk) && bit(&self.in_flight, self.slots, chunk)
    }

    /// A copy of `chunk` left, or is no longer in flight.
    pub(crate) fn set_in_flight(&mut self, chunk: u64, in_flight: bool) {
        if (self.base..self.end()).contains(&chunk) {
            set_bit(&mut self.in_flight, self.slots, chunk, in_flight);
        }
    }

    /// `chunk` was delivered; returns `false` if it already was. The window slides past every
    /// chunk delivered from its base on.
    pub(crate) fn set_delivered(&mut self, chunk: u64) -> bool {
        if self.is_delivered(chunk) || chunk >= self.end() {
            return false;
        }
        set_bit(&mut self.delivered, self.slots, chunk, true);
        while bit(&self.delivered, self.slots, self.base) {
            set_bit(&mut self.delivered, self.slots, self.base, false);
            set_bit(&mut self.in_flight, self.slots, self.base, false);
            self.base += 1;
        }
        true
    }
}

fn position(slots: u64, chunk: u64) -> (usize, u64) {
    let slot = chunk % slots;
    (
        usize::try_from(slot / BITS_PER_WORD).unwrap_or(usize::MAX),
        1 << (slot % BITS_PER_WORD),
    )
}

fn bit(words: &[u64], slots: u64, chunk: u64) -> bool {
    let (word, mask) = position(slots, chunk);
    words.get(word).is_some_and(|word| word & mask != 0)
}

fn set_bit(words: &mut [u64], slots: u64, chunk: u64, value: bool) {
    let (word, mask) = position(slots, chunk);
    if let Some(word) = words.get_mut(word) {
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
    }
}
