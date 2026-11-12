//! A named day on the road: one path per SIM, optionally on a shared tower.

use crate::{GilbertElliott, Path};

/// A named set of SIMs, optionally on the same tower.
#[derive(Debug, Clone)]
pub struct Scenario {
    /// Short identifier, used in benchmark reports.
    pub name: &'static str,
    /// Length of the scenario.
    pub duration_ms: u64,
    /// One path per SIM.
    pub paths: Vec<Path>,
    /// Congestion shared by every path when they sit on the same cell.
    pub shared_tower: Option<GilbertElliott>,
}
