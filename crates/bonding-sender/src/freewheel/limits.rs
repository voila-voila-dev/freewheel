//! Timers and bounds of the freewheel sender. Times are in microseconds.

/// Housekeeping (joins, keepalives, stall checks) runs at most this often.
pub const HOUSEKEEPING_PERIOD_US: u64 = 10_000;
/// A link not accepted yet sends a join this often.
pub const JOIN_RETRY_US: u64 = 250_000;
/// A link whose socket could not be opened is retried this often (SRTLA mode: every second).
pub const OPEN_RETRY_US: u64 = 1_000_000;
/// A link that sent nothing for this long sends a keepalive, so its RTT stays measured and a
/// link that comes back is seen within a few tens of milliseconds.
pub const KEEPALIVE_IDLE_US: u64 = 50_000;
/// A link that heard no ACK for this many smoothed RTTs, while it had sent something since,
/// has stalled: no more data until an ACK comes back.
pub const STALL_RTT_MULTIPLE: u64 = 4;
/// ...and never sooner than this, so jitter on a short RTT does not stall a healthy link.
pub const STALL_MIN_US: u64 = 300_000;
/// ...and never later than this, even when a bloated queue has stretched the RTT: a link
/// silent for a second is not carrying the live stream.
pub const STALL_MAX_US: u64 = 1_000_000;
/// A live packet expected to arrive later than this is dropped at the box instead of being
/// queued in a modem: half of SRT's 2 s latency, so one retransmission still fits. Queuing it
/// would only delay every packet behind it (`trace-drive`: seconds of stale queue after an
/// outage).
pub const MAX_EXPECTED_ARRIVAL_US: u64 = 1_000_000;
/// When every link has filled its congestion window, a live packet still goes out on the
/// earliest link only if it would arrive within this: beyond, it would sit in a modem queue
/// that SRT reads as clock drift and answers by delaying the whole stream.
pub const MAX_OVERFLOW_ARRIVAL_US: u64 = 500_000;
/// RTT assumed before a link has measured one (a typical LTE uplink).
pub const INITIAL_RTT_US: u64 = 100_000;
/// Rate assumed for a link that has delivered little yet, and the floor of every estimate,
/// in bit/s: low enough not to flood a weak modem, high enough to try it.
pub const MIN_RATE_BPS: u64 = 500_000;
/// Smallest congestion window, in bytes: ten full packets.
pub const MIN_CONGESTION_WINDOW_BYTES: u64 = 10 * 1_356;
/// A delivery rate sample covers at least this long.
pub const RATE_SAMPLE_PERIOD_US: u64 = 100_000;
/// Samples the rate estimate keeps (its maximum is used): one second of history.
pub const RATE_SAMPLES: usize = 10;
/// Packets one link remembers until they are acknowledged: 300 ms at 50 Mbit/s.
pub const IN_FLIGHT_CAPACITY: usize = 2_048;
