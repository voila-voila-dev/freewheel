//! Power drawn by a cellular radio in its RRC states.

/// Radio power model with RRC states: active while transmitting, a "tail" in the
/// connected state after the last activity, then idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioPower {
    /// Power while idle, in milliwatts.
    pub idle_mw: u32,
    /// Power during a millisecond with traffic, in milliwatts.
    pub active_mw: u32,
    /// Power while connected without traffic, in milliwatts.
    pub tail_mw: u32,
    /// How long the radio stays connected after the last activity.
    pub tail_ms: u64,
}

impl RadioPower {
    /// Order of magnitude for LTE, from published smartphone measurements
    /// (Huang et al., `MobiSys` 2012). To be replaced by our own measurements on the box.
    #[must_use]
    pub const fn lte_typical() -> Self {
        Self {
            idle_mw: 30,
            active_mw: 2_000,
            tail_mw: 1_000,
            tail_ms: 11_500,
        }
    }
}
