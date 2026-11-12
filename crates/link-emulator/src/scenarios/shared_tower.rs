//! Two SIMs on the same mast: they degrade together.

use super::build_path::build_path;
use super::minute::MINUTE;
use super::scenario::Scenario;
use crate::{GilbertElliott, RateModel, TraceError};

/// Two SIMs from different operators on the same mast: they degrade together.
///
/// # Errors
/// Fails only if a generated trace is empty.
pub fn shared_tower(seed: u64) -> Result<Scenario, TraceError> {
    let duration_ms = 20 * MINUTE;
    let uplink = RateModel::markov(500, &[1.0, 4.0, 7.0], 10_000);
    let loss = GilbertElliott::bernoulli(0.002);
    let a = build_path(&uplink, duration_ms, 40, loss, &[], seed)?;
    let b = build_path(&uplink, duration_ms, 40, loss, &[], seed.wrapping_add(7))?;
    Ok(Scenario {
        name: "shared_tower",
        duration_ms,
        paths: vec![a, b],
        shared_tower: Some(GilbertElliott::new(0.0005, 0.01, 0.0, 0.5)),
    })
}
