//! The recording upload's state (M1b step 6, the background class): what of the recording is
//! delivered, what is on its way, and what to send next.

use bonding_protocol::freewheel::SequenceRange;

use super::chunk_window::ChunkWindow;
use super::chunks_in_flight::{ChunkInFlight, ChunksInFlight};
use super::recording_settings::RecordingSettings;

/// Bytes of the recording to send next: from `offset`, `len` of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkToSend {
    /// Where they start in the recording.
    pub offset: u64,
    /// How many: the chunk size, or less for the recording's tail.
    pub len: usize,
}

/// The upload of one recording, a file that may still be growing, cut into chunks of
/// [`RecordingSettings::chunk_bytes`]: chunk `i` holds bytes `i × size` to `(i + 1) × size`.
///
/// It sends the first chunk neither delivered nor in flight, within
/// [`RecordingSettings::window_bytes`] of the first one not delivered. A chunk is delivered when
/// an ACK's ranges hold its sequence; it is lost (and sent again) when an ACK of its link
/// acknowledges a later sequence without it, when its link stalls, or after
/// [`RecordingSettings::loss_timeout_us`]. All of it lives in memory for the whole session, so
/// an outage only pauses it. The recording has ended once it stops growing for
/// [`RecordingSettings::end_idle_us`]; then its last, partial chunk goes too, and after
/// [`RecordingSettings::drain_us`] more the upload gives up. Allocated once, when created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingUpload {
    chunk_bytes: u64,
    loss_timeout_us: u64,
    end_idle_us: u64,
    drain_us: u64,
    window: ChunkWindow,
    in_flight: ChunksInFlight,
    /// Every chunk before this one is delivered or in flight.
    scan_from: u64,
    recording_bytes: u64,
    grew_at_us: Option<u64>,
    ended_at_us: Option<u64>,
    /// The partial last chunk delivered so far: its index, and the offset past its bytes.
    tail_delivered: Option<(u64, u64)>,
    full_chunks_delivered: u64,
    lost_chunks: u64,
    sent_chunks: u64,
}

impl RecordingUpload {
    /// An upload with nothing of the recording seen yet.
    #[must_use]
    pub fn new(settings: &RecordingSettings) -> Self {
        let chunk_bytes = (settings.chunk_bytes as u64).max(1);
        Self {
            chunk_bytes,
            loss_timeout_us: settings.loss_timeout_us,
            end_idle_us: settings.end_idle_us,
            drain_us: settings.drain_us,
            window: ChunkWindow::new(settings.window_bytes / chunk_bytes),
            in_flight: ChunksInFlight::new(settings.in_flight_chunks),
            scan_from: 0,
            recording_bytes: 0,
            grew_at_us: None,
            ended_at_us: None,
            tail_delivered: None,
            full_chunks_delivered: 0,
            lost_chunks: 0,
            sent_chunks: 0,
        }
    }

    /// The recording is `bytes` long at `now_us` (it only grows: a shorter length is ignored).
    /// Returns `true` when this call finds it ended: it has not grown for
    /// [`RecordingSettings::end_idle_us`].
    pub fn on_recording_bytes(&mut self, now_us: u64, bytes: u64) -> bool {
        if bytes > self.recording_bytes || self.grew_at_us.is_none() {
            if bytes > self.recording_bytes {
                self.ended_at_us = None;
            }
            self.recording_bytes = self.recording_bytes.max(bytes);
            self.grew_at_us = Some(now_us);
            return false;
        }
        let idle = self
            .grew_at_us
            .is_some_and(|at_us| now_us.saturating_sub(at_us) >= self.end_idle_us);
        if idle && self.ended_at_us.is_none() {
            self.ended_at_us = Some(now_us);
            return true;
        }
        false
    }

    /// The next bytes to send at `now_us`, if any: none while the window or the in-flight
    /// ring is full, everything seen is delivered or on its way, or the upload gave up. Until
    /// [`on_sent`](Self::on_sent) says they left, the same bytes come again.
    pub fn next_chunk(&mut self, now_us: u64) -> Option<ChunkToSend> {
        if self.in_flight.is_full() || self.abandoned(now_us) {
            return None;
        }
        let full_chunks = self.recording_bytes / self.chunk_bytes;
        let tail_bytes = self.recording_bytes % self.chunk_bytes;
        let sendable_end = if self.ended_at_us.is_some() && tail_bytes > 0 {
            full_chunks + 1
        } else {
            full_chunks
        };
        let start = self.scan_from.max(self.window.base());
        let end = sendable_end.min(self.window.end());
        let chunk = (start..end).find(|&chunk| {
            let done = self.window.is_delivered(chunk)
                || self.window.is_in_flight(chunk)
                || (chunk == full_chunks && self.tail_done());
            !done
        });
        let Some(chunk) = chunk else {
            self.scan_from = end;
            return None;
        };
        // Not past it: if it does not leave, it is the next one again.
        self.scan_from = chunk;
        let offset = chunk * self.chunk_bytes;
        let len = (self.recording_bytes - offset).min(self.chunk_bytes);
        Some(ChunkToSend {
            offset,
            len: usize::try_from(len).unwrap_or(0),
        })
    }

    /// `chunk` (from [`next_chunk`](Self::next_chunk)) left as `sequence` on `link_id`.
    pub fn on_sent(&mut self, chunk: ChunkToSend, sequence: u32, link_id: u8, now_us: u64) {
        let index = chunk.offset / self.chunk_bytes;
        let entry = ChunkInFlight {
            sequence,
            link_id,
            chunk: index,
            len: u16::try_from(chunk.len).unwrap_or(u16::MAX),
            sent_at_us: now_us,
            resolved: false,
        };
        if self.in_flight.push(entry) {
            self.window.set_in_flight(index, true);
            self.sent_chunks += 1;
        }
    }

    /// An ACK's ranges: the chunks sent with those sequences are delivered.
    pub fn on_acknowledged(&mut self, ranges: impl Iterator<Item = SequenceRange>) {
        for range in ranges {
            for position in self.in_flight.positions_between(range.first, range.last) {
                self.resolve(position, true);
            }
        }
        self.in_flight.drop_resolved_front();
    }

    /// An ACK of `link_id` acknowledged `largest_sequence`: the chunks sent on that link before
    /// it and not delivered (call [`on_acknowledged`](Self::on_acknowledged) first) are lost.
    pub fn on_link_acknowledged(&mut self, link_id: u8, largest_sequence: u32) {
        let Some(front) = self.in_flight.get(0).map(|entry| entry.sequence) else {
            return;
        };
        for position in self.in_flight.positions_between(front, largest_sequence) {
            if self
                .in_flight
                .get(position)
                .is_some_and(|entry| entry.link_id == link_id)
            {
                self.resolve(position, false);
            }
        }
        self.in_flight.drop_resolved_front();
    }

    /// `link_id` stalled, closed or left: what it had in flight is lost.
    pub fn on_link_lost(&mut self, link_id: u8) {
        for position in 0..self.in_flight.len() {
            if self
                .in_flight
                .get(position)
                .is_some_and(|entry| entry.link_id == link_id)
            {
                self.resolve(position, false);
            }
        }
        self.in_flight.drop_resolved_front();
    }

    /// Chunks in flight for [`RecordingSettings::loss_timeout_us`] at `now_us` are lost.
    pub fn expire(&mut self, now_us: u64) {
        for position in 0..self.in_flight.len() {
            let Some(entry) = self.in_flight.get(position) else {
                break;
            };
            if entry.resolved {
                continue;
            }
            if now_us.saturating_sub(entry.sent_at_us) < self.loss_timeout_us {
                break;
            }
            self.resolve(position, false);
        }
        self.in_flight.drop_resolved_front();
    }

    /// Bytes of the recording seen so far.
    #[must_use]
    pub const fn recording_bytes(&self) -> u64 {
        self.recording_bytes
    }

    /// Bytes of the recording the receiver has.
    #[must_use]
    pub fn delivered_bytes(&self) -> u64 {
        let tail = self
            .tail_delivered
            .filter(|(chunk, _)| !self.window.is_delivered(*chunk))
            .map_or(0, |(chunk, end)| end - chunk * self.chunk_bytes);
        self.full_chunks_delivered * self.chunk_bytes + tail
    }

    /// The recording has ended (it stopped growing).
    #[must_use]
    pub const fn ended(&self) -> bool {
        self.ended_at_us.is_some()
    }

    /// The recording has ended and the receiver has all of it.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.ended() && self.delivered_bytes() == self.recording_bytes
    }

    /// The recording ended over [`RecordingSettings::drain_us`] ago: nothing more is sent.
    #[must_use]
    pub fn abandoned(&self, now_us: u64) -> bool {
        self.ended_at_us
            .is_some_and(|at_us| now_us.saturating_sub(at_us) >= self.drain_us)
    }

    /// Chunks sent so far, resends included.
    #[must_use]
    pub const fn sent_chunks(&self) -> u64 {
        self.sent_chunks
    }

    /// Chunk copies found lost so far.
    #[must_use]
    pub const fn lost_chunks(&self) -> u64 {
        self.lost_chunks
    }

    /// Chunks in flight, resolved ones not yet forgotten included.
    #[must_use]
    pub const fn chunks_in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// The tail chunk (partial) is already delivered whole, as the recording now stands.
    fn tail_done(&self) -> bool {
        self.tail_delivered
            .is_some_and(|(_, end)| end == self.recording_bytes)
    }

    /// The copy at `position` was delivered, or is lost.
    fn resolve(&mut self, position: usize, delivered: bool) {
        let Some(entry) = self.in_flight.get_mut(position) else {
            return;
        };
        if entry.resolved {
            return;
        }
        entry.resolved = true;
        let entry = *entry;
        self.window.set_in_flight(entry.chunk, false);
        let full = u64::from(entry.len) == self.chunk_bytes;
        if !delivered {
            self.lost_chunks += 1;
        } else if full {
            if self.window.set_delivered(entry.chunk) {
                self.full_chunks_delivered += 1;
            }
        } else {
            let end = entry.chunk * self.chunk_bytes + u64::from(entry.len);
            if self.tail_delivered.is_none_or(|(_, known)| end > known) {
                self.tail_delivered = Some((entry.chunk, end));
            }
        }
        if !delivered || !full {
            // Lost, or a tail the recording may have outgrown: look at it again.
            self.scan_from = self.scan_from.min(entry.chunk);
        }
    }
}
