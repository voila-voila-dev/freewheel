//! Limits and timers of the freewheel receiver.

/// Sessions served at once (one per box streaming); SRTLA mode allows 200 groups, a box
/// needs one.
pub const MAX_SESSIONS: usize = 64;
/// Links one session may have: more modems than any box carries.
pub const MAX_LINKS_PER_SESSION: usize = 16;
/// A link acknowledges after this many data packets: often enough for the sender to see a
/// change within a few milliseconds at 3 Mbit/s, without an ACK per packet at 50.
pub const ACK_EVERY_PACKETS: u32 = 4;
/// Longest a received packet waits for its ACK, in microseconds, when traffic is too thin
/// to reach [`ACK_EVERY_PACKETS`].
pub const ACK_MAX_DELAY_US: u64 = 5_000;
/// A link silent for this long is forgotten. The sender keeps idle links alive every 50 ms,
/// so this only catches links that are gone.
pub const LINK_TIMEOUT_US: u64 = 10_000_000;
/// A session without links for this long is removed, with its SRT socket.
pub const SESSION_TIMEOUT_US: u64 = 10_000_000;
/// Sequences a session remembers below the highest received, to drop duplicates and report
/// holes: 3 s of a 3.4 Mbit/s stream, 300 ms at 40 Mbit/s, beyond any retransmission that
/// still matters at the stream rates we carry.
pub const RECEIVED_WINDOW_SEQUENCES: u32 = 1_024;
/// Most runs of received sequences one ACK reports, newest first: enough for the holes left
/// by the packets other links still carry, at 540 bytes on the downlink.
pub const ACK_RANGES: usize = 64;
