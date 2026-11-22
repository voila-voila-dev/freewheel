//! Which of a session's recent sequences arrived: to drop duplicates and to tell the sender
//! what is missing.

use bonding_protocol::freewheel::SequenceRange;

use super::limits::{ACK_RANGES, RECEIVED_WINDOW_SEQUENCES};

const BITS_PER_WORD: usize = 64;
const WINDOW: u32 = RECEIVED_WINDOW_SEQUENCES;
const WORDS: usize = WINDOW as usize / BITS_PER_WORD;

/// What one arrival was, against what came before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// First time seen, or too old to tell: forward it.
    New,
    /// Already received: a retransmission whose original made it after all.
    Duplicate,
}

/// The last [`RECEIVED_WINDOW_SEQUENCES`] sequences below the highest received, one bit each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedWindow {
    highest: Option<u32>,
    bits: [u64; WORDS],
}

impl Default for ReceivedWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceivedWindow {
    /// Nothing received yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            highest: None,
            bits: [0; WORDS],
        }
    }

    /// Records `sequence`; says whether it is new.
    pub fn record(&mut self, sequence: u32) -> Arrival {
        let Some(highest) = self.highest else {
            self.highest = Some(sequence);
            self.set(sequence);
            return Arrival::New;
        };
        let ahead = sequence.wrapping_sub(highest);
        if ahead != 0 && ahead < u32::MAX / 2 {
            if ahead >= WINDOW {
                self.bits = [0; WORDS];
            } else {
                for skipped in 1..=ahead {
                    self.clear(highest.wrapping_add(skipped));
                }
            }
            self.highest = Some(sequence);
            self.set(sequence);
            return Arrival::New;
        }
        if highest.wrapping_sub(sequence) >= WINDOW {
            return Arrival::New;
        }
        if self.get(sequence) {
            return Arrival::Duplicate;
        }
        self.set(sequence);
        Arrival::New
    }

    /// The runs of received sequences at and below `from`, newest last, at most
    /// [`ACK_RANGES`] of them, written into `out`; returns how many. Nothing when `from` has
    /// left the window.
    pub fn ranges_below(&self, from: u32, out: &mut [SequenceRange; ACK_RANGES]) -> usize {
        let Some(highest) = self.highest else {
            return 0;
        };
        let depth = highest.wrapping_sub(from);
        if depth >= WINDOW {
            return 0;
        }
        let mut count = 0;
        let mut run_last: Option<u32> = None;
        let mut sequence = from;
        for _ in depth..WINDOW {
            match (self.get(sequence), run_last) {
                (true, None) => run_last = Some(sequence),
                (false, Some(last)) => {
                    if let Some(slot) = out.get_mut(count) {
                        *slot = SequenceRange {
                            first: sequence.wrapping_add(1),
                            last,
                        };
                    }
                    count += 1;
                    run_last = None;
                    if count == ACK_RANGES {
                        break;
                    }
                }
                _ => {}
            }
            sequence = sequence.wrapping_sub(1);
        }
        if let (Some(last), true) = (run_last, count < ACK_RANGES) {
            if let Some(slot) = out.get_mut(count) {
                *slot = SequenceRange {
                    first: sequence.wrapping_add(1),
                    last,
                };
            }
            count += 1;
        }
        if let Some(written) = out.get_mut(..count) {
            written.reverse();
        }
        count
    }

    fn slot(sequence: u32) -> (usize, u64) {
        let index = (sequence % WINDOW) as usize;
        (index / BITS_PER_WORD, 1 << (index % BITS_PER_WORD))
    }

    fn get(&self, sequence: u32) -> bool {
        let (word, mask) = Self::slot(sequence);
        self.bits.get(word).is_some_and(|bits| bits & mask != 0)
    }

    fn set(&mut self, sequence: u32) {
        let (word, mask) = Self::slot(sequence);
        if let Some(bits) = self.bits.get_mut(word) {
            *bits |= mask;
        }
    }

    fn clear(&mut self, sequence: u32) {
        let (word, mask) = Self::slot(sequence);
        if let Some(bits) = self.bits.get_mut(word) {
            *bits &= !mask;
        }
    }
}
