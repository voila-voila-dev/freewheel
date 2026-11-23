//! Whether the live stream is running, for the recording upload.

const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const BITS_PER_BYTE: u64 = 8;

/// Live bytes sent per second, over the last whole second: the live stream is idle (it ended,
/// or has not started) when that is below a threshold, and then the recording upload may let
/// a link's queue grow, since no live packet waits in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LiveActivity {
    second_started_us: Option<u64>,
    bytes_this_second: u64,
    bytes_last_second: u64,
}

impl LiveActivity {
    /// Nothing sent yet.
    pub(crate) const fn new() -> Self {
        Self {
            second_started_us: None,
            bytes_this_second: 0,
            bytes_last_second: 0,
        }
    }

    /// `bytes` of live data left at `now_us`.
    pub(crate) fn on_sent(&mut self, now_us: u64, bytes: u64) {
        self.roll(now_us);
        self.bytes_this_second += bytes;
    }

    /// The live stream sent less than `idle_bps` over the last whole second at `now_us`.
    pub(crate) fn idle(&mut self, now_us: u64, idle_bps: u64) -> bool {
        self.roll(now_us);
        self.bytes_last_second * BITS_PER_BYTE < idle_bps
    }

    fn roll(&mut self, now_us: u64) {
        let started_us = *self.second_started_us.get_or_insert(now_us);
        let elapsed_us = now_us.saturating_sub(started_us);
        if elapsed_us >= MICROSECONDS_PER_SECOND {
            self.bytes_last_second = if elapsed_us >= 2 * MICROSECONDS_PER_SECOND {
                0
            } else {
                self.bytes_this_second
            };
            self.bytes_this_second = 0;
            self.second_started_us = Some(now_us);
        }
    }
}
