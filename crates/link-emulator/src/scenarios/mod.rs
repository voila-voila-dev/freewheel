//! Ready-made days on the road. Every scenario is deterministic for a given seed.

mod build_path;
mod desert_day;
mod minute;
mod mountain_tunnels;
mod saturated_city;
mod scenario;
mod shared_tower;

pub use desert_day::desert_day;
pub use mountain_tunnels::mountain_tunnels;
pub use saturated_city::saturated_city;
pub use scenario::Scenario;
pub use shared_tower::shared_tower;

use crate::TraceError;

/// Every scenario, for benchmark suites.
///
/// # Errors
/// Fails only if a generated trace is empty.
pub fn all(seed: u64) -> Result<Vec<Scenario>, TraceError> {
    Ok(vec![
        desert_day(seed)?,
        saturated_city(seed)?,
        mountain_tunnels(seed)?,
        shared_tower(seed)?,
    ])
}
