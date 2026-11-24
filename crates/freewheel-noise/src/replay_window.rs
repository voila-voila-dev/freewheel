//! Which nonces a session has already opened, over a sliding window.

/// Default [`ReplayWindow`] size, in packets: the largest reordering between links, 500 ms of
/// queuing (the sender's `MAX_OVERFLOW_ARRIVAL_US`), at 196 Mbit/s of full datagrams, several
/// times what a box's links carry. 1 KiB per session and direction.
pub const DEFAULT_REPLAY_WINDOW_PACKETS: usize = 8_192;
/// Largest [`ReplayWindow`] size, in packets: 128 KiB per session and direction.
pub const MAX_REPLAY_WINDOW_PACKETS: usize = 1 << 20;

const BITS_PER_WORD: u64 = 64;

/// The nonces seen among the latest `packets` (rounded up to a multiple of 64, at least 64):
/// a nonce below the window, or already seen within it, is a replay. Links reorder packets
/// (several links carry one session), so nonces may come in any order within the window.
/// Allocated once, when made; nothing allocates after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayWindow {
    /// One bit per nonce, nonce `n` at bit `n % size`.
    seen: Vec<u64>,
    /// The highest nonce seen, if any.
    highest: Option<u64>,
}

impl ReplayWindow {
    /// A window over the latest `packets` nonces, none seen yet.
    #[must_use]
    pub fn new(packets: usize) -> Self {
        let words = packets.div_ceil(64).max(1);
        Self {
            seen: vec![0; words],
            highest: None,
        }
    }

    /// How many nonces it spans.
    #[must_use]
    pub fn packets(&self) -> usize {
        self.seen.len() * 64
    }

    /// `true` when `nonce` may be opened: above every nonce seen, or within the window and not
    /// seen yet.
    #[must_use]
    pub fn is_fresh(&self, nonce: u64) -> bool {
        let Some(highest) = self.highest else {
            return true;
        };
        if nonce > highest {
            return true;
        }
        highest - nonce < self.size() && !self.bit(nonce)
    }

    /// Records `nonce` as opened (after its tag checked out, and [`is_fresh`](Self::is_fresh)):
    /// a nonce above the highest slides the window up to it.
    pub fn mark(&mut self, nonce: u64) {
        match self.highest {
            Some(highest) if nonce <= highest => {}
            Some(highest) if nonce - highest < self.size() => {
                for passed in highest + 1..nonce {
                    self.set(passed, false);
                }
                self.highest = Some(nonce);
            }
            _ => {
                self.seen.fill(0);
                self.highest = Some(nonce);
            }
        }
        self.set(nonce, true);
    }

    /// Forgets every nonce: a new handshake, new keys.
    pub fn reset(&mut self) {
        self.seen.fill(0);
        self.highest = None;
    }

    fn size(&self) -> u64 {
        self.seen.len() as u64 * BITS_PER_WORD
    }

    fn position(&self, nonce: u64) -> (usize, u64) {
        let index = nonce % self.size();
        (
            usize::try_from(index / BITS_PER_WORD).unwrap_or_default(),
            1 << (index % BITS_PER_WORD),
        )
    }

    fn bit(&self, nonce: u64) -> bool {
        let (word, mask) = self.position(nonce);
        self.seen.get(word).is_some_and(|bits| bits & mask != 0)
    }

    fn set(&mut self, nonce: u64, on: bool) {
        let (word, mask) = self.position(nonce);
        if let Some(bits) = self.seen.get_mut(word) {
            if on {
                *bits |= mask;
            } else {
                *bits &= !mask;
            }
        }
    }
}
