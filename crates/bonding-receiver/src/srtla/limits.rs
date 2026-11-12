//! Upstream `srtla_rec`'s limits and timeouts (`srtla.md`, "Timeouts and limits").

/// Links one group accepts; the next one gets `REG_ERR`.
pub const MAX_LINKS_PER_GROUP: usize = 8;
/// Groups the receiver holds at once; the next REG1 gets `REG_ERR`.
pub const MAX_GROUPS: usize = 200;
/// How often links and groups are checked for timeouts, in seconds.
pub const CLEANUP_PERIOD_S: u64 = 3;
/// A link that sent nothing for longer than this is forgotten (checked every
/// [`CLEANUP_PERIOD_S`], so after 10–13 s).
pub const LINK_TIMEOUT_S: u64 = 10;
/// A group without links is removed once it is older than this.
pub const EMPTY_GROUP_TIMEOUT_S: u64 = 10;
/// Data packets per SRTLA ACK: one ACK every this many packets on a link, never a partial one.
pub const ACK_BATCH_LEN: usize = 10;
