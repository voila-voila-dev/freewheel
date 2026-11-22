//! Packets sent on one link and not acknowledged yet.

use super::limits::IN_FLIGHT_CAPACITY;

/// A ring of the packets one link has in flight, oldest first: their sequences and sizes.
/// Allocated once, with the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InFlight {
    sequences: Vec<u32>,
    sizes_bytes: Vec<u16>,
    oldest: usize,
    count: usize,
    total_bytes: u64,
}

impl Default for InFlight {
    fn default() -> Self {
        Self::new()
    }
}

impl InFlight {
    /// An empty ring of [`IN_FLIGHT_CAPACITY`] packets.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sequences: vec![0; IN_FLIGHT_CAPACITY],
            sizes_bytes: vec![0; IN_FLIGHT_CAPACITY],
            oldest: 0,
            count: 0,
            total_bytes: 0,
        }
    }

    /// Bytes in flight.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Packets in flight.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// `true` when nothing is in flight.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Remembers a packet just sent; when full, the oldest is forgotten (counted as gone).
    pub fn push(&mut self, sequence: u32, size_bytes: u16) {
        if self.count == IN_FLIGHT_CAPACITY {
            let _ = self.pop_oldest();
        }
        let slot = (self.oldest + self.count) % IN_FLIGHT_CAPACITY;
        if let (Some(stored), Some(size)) =
            (self.sequences.get_mut(slot), self.sizes_bytes.get_mut(slot))
        {
            *stored = sequence;
            *size = size_bytes;
            self.count += 1;
            self.total_bytes += u64::from(size_bytes);
        }
    }

    /// `sequence` arrived: it and every packet sent before it on this link are out of flight.
    /// Those `lost` says are missing are added to `lost_out` (up to its capacity, so it never
    /// grows): a later packet of the same link made it, so they will not. Returns the bytes
    /// out of flight; 0 if `sequence` is not in flight.
    pub fn acknowledge(
        &mut self,
        sequence: u32,
        lost: impl Fn(u32) -> bool,
        lost_out: &mut Vec<u32>,
    ) -> u64 {
        let Some(position) = (0..self.count).find(|offset| {
            self.sequences
                .get((self.oldest + offset) % IN_FLIGHT_CAPACITY)
                == Some(&sequence)
        }) else {
            return 0;
        };
        let mut bytes = 0;
        for _ in 0..=position {
            let (oldest, size) = self.pop_oldest();
            bytes += size;
            if oldest != sequence && lost(oldest) && lost_out.len() < lost_out.capacity() {
                lost_out.push(oldest);
            }
        }
        bytes
    }

    /// Forgets everything (the link rejoined).
    pub fn clear(&mut self) {
        self.oldest = 0;
        self.count = 0;
        self.total_bytes = 0;
    }

    /// Takes everything out of flight into `lost_out` (up to its capacity): the link
    /// stalled, so none of it is expected any more.
    pub fn drain(&mut self, lost_out: &mut Vec<u32>) {
        while self.count > 0 {
            let (oldest, _) = self.pop_oldest();
            if lost_out.len() < lost_out.capacity() {
                lost_out.push(oldest);
            }
        }
        self.clear();
    }

    /// Removes the oldest packet: its sequence and size.
    fn pop_oldest(&mut self) -> (u32, u64) {
        if self.count == 0 {
            return (0, 0);
        }
        let sequence = self.sequences.get(self.oldest).copied().unwrap_or(0);
        let size = u64::from(self.sizes_bytes.get(self.oldest).copied().unwrap_or(0));
        self.oldest = (self.oldest + 1) % IN_FLIGHT_CAPACITY;
        self.count -= 1;
        self.total_bytes -= size;
        (sequence, size)
    }
}
