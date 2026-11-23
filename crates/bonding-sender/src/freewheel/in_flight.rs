//! Packets sent on one link and not acknowledged yet.

use super::limits::IN_FLIGHT_CAPACITY;

/// A ring of the packets one link has in flight, oldest first: their sequences and sizes, and
/// which are the recording upload's spare-capacity traffic (M1b step 6), which the live
/// traffic does not count. Allocated once, with the link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InFlight {
    sequences: Vec<u32>,
    sizes_bytes: Vec<u16>,
    spare: Vec<bool>,
    oldest: usize,
    count: usize,
    total_bytes: u64,
    spare_bytes: u64,
    /// Spare-capacity bytes taken out of flight since the last
    /// [`take_left_spare_bytes`](Self::take_left_spare_bytes).
    left_spare_bytes: u64,
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
            spare: vec![false; IN_FLIGHT_CAPACITY],
            oldest: 0,
            count: 0,
            total_bytes: 0,
            spare_bytes: 0,
            left_spare_bytes: 0,
        }
    }

    /// Bytes in flight.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Bytes in flight the live traffic counts: all but the recording upload's on spare
    /// capacity, which yields to it.
    #[must_use]
    pub const fn live_view_bytes(&self) -> u64 {
        self.total_bytes - self.spare_bytes
    }

    /// Bytes in flight of the recording upload on spare capacity.
    #[must_use]
    pub const fn spare_bytes(&self) -> u64 {
        self.spare_bytes
    }

    /// Spare-capacity bytes taken out of flight (acknowledged or lost) since the last call.
    pub const fn take_left_spare_bytes(&mut self) -> u64 {
        let bytes = self.left_spare_bytes;
        self.left_spare_bytes = 0;
        bytes
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
        self.push_packet(sequence, size_bytes, false);
    }

    /// Remembers a packet of the recording upload just sent on spare capacity.
    pub fn push_spare(&mut self, sequence: u32, size_bytes: u16) {
        self.push_packet(sequence, size_bytes, true);
    }

    fn push_packet(&mut self, sequence: u32, size_bytes: u16, spare: bool) {
        if self.count == IN_FLIGHT_CAPACITY {
            self.pop_oldest();
        }
        let slot = (self.oldest + self.count) % IN_FLIGHT_CAPACITY;
        if let (Some(stored), Some(size), Some(stored_spare)) = (
            self.sequences.get_mut(slot),
            self.sizes_bytes.get_mut(slot),
            self.spare.get_mut(slot),
        ) {
            *stored = sequence;
            *size = size_bytes;
            *stored_spare = spare;
            self.count += 1;
            self.total_bytes += u64::from(size_bytes);
            if spare {
                self.spare_bytes += u64::from(size_bytes);
            }
        }
    }

    /// `sequence` arrived: it and every packet sent before it on this link are out of flight
    /// (delivered or lost). Returns their bytes; 0 if `sequence` is not in flight.
    pub fn acknowledge(&mut self, sequence: u32) -> u64 {
        let Some(position) = (0..self.count).find(|offset| {
            self.sequences
                .get((self.oldest + offset) % IN_FLIGHT_CAPACITY)
                == Some(&sequence)
        }) else {
            return 0;
        };
        (0..=position).map(|_| self.pop_oldest()).sum()
    }

    /// Forgets everything (the link stalled or rejoined).
    pub fn clear(&mut self) {
        self.oldest = 0;
        self.count = 0;
        self.total_bytes = 0;
        self.spare_bytes = 0;
    }

    fn pop_oldest(&mut self) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let size = u64::from(self.sizes_bytes.get(self.oldest).copied().unwrap_or(0));
        if self.spare.get(self.oldest).copied().unwrap_or(false) {
            self.spare_bytes -= size;
            self.left_spare_bytes += size;
        }
        self.oldest = (self.oldest + 1) % IN_FLIGHT_CAPACITY;
        self.count -= 1;
        self.total_bytes -= size;
        size
    }
}
