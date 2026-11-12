//! Upstream `srtla_send`'s limits, timeouts and window steps (`srtla.md`, "Link selection" and
//! "Timeouts and limits"). Windows are in thousandths of a packet, as upstream counts them.

/// Sequence numbers each link remembers; the oldest slot is never searched (upstream scans
/// from the newest and stops one short of a full turn).
pub const PACKET_LOG_LEN: usize = 256;
/// A link that received nothing for longer than this, in whole seconds, has failed (so after
/// 4–5 s).
pub const LINK_TIMEOUT_S: u64 = 4;
/// How long a REG1 waits for its REG2, and how long `REG_NGP` is ignored after a group is
/// registered, in seconds.
pub const REGISTRATION_TIMEOUT_S: u64 = 4;
/// With every link failed for longer than this, the sender gives up (or, before it ever
/// connected, tries the receiver's next address).
pub const GIVE_UP_AFTER_MS: u64 = 10_000;
/// A link that sent nothing for longer than this, in whole seconds, sends a keepalive.
pub const IDLE_BEFORE_KEEPALIVE_S: u64 = 1;
/// Housekeeping runs at most this often.
pub const HOUSEKEEPING_PERIOD_MS: u64 = 1_000;
/// Smallest window, also the window of a link that just failed.
pub const WINDOW_MIN_MILLIPACKETS: u32 = 1_000;
/// Window of a new link.
pub const WINDOW_DEFAULT_MILLIPACKETS: u32 = 20_000;
/// Largest window reached by SRTLA ACKs.
pub const WINDOW_MAX_MILLIPACKETS: u32 = 60_000;
/// Taken from a link for each of its packets an SRT NAK reports lost.
pub const WINDOW_NAK_DECREASE_MILLIPACKETS: u32 = 100;
/// Added to a link for an SRTLA ACK of one of its packets while it has more in flight than its
/// window (upstream's 30, minus the 1 every link gets for that ACK).
pub const WINDOW_ACK_INCREASE_MILLIPACKETS: u32 = 29;
/// Added to every link that ever heard from the receiver, for each number of an SRTLA ACK.
pub(crate) const WINDOW_ACK_BROADCAST_INCREASE_MILLIPACKETS: u32 = 1;
/// One packet, in the window's unit.
pub(crate) const MILLIPACKETS_PER_PACKET: u64 = 1_000;
