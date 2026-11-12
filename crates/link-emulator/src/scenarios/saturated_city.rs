//! Crowded city: bursty loss and frequent handovers on two SIMs.

use super::build_path::build_path;
use super::minute::MINUTE;
use super::scenario::Scenario;
use crate::{Event, GilbertElliott, RateModel, TraceError};

/// Crowded city: decent capacity, heavy bursty loss, frequent handovers, two SIMs.
///
/// # Errors
/// Fails only if a generated trace is empty.
pub fn saturated_city(seed: u64) -> Result<Scenario, TraceError> {
    let duration_ms = 30 * MINUTE;
    let uplink = RateModel::markov(200, &[1.0, 3.0, 6.0, 10.0], 5_000);
    let handovers: Vec<Event> = (1..30)
        .map(|minute| Event::Handover {
            at_ms: minute * MINUTE,
            interruption_ms: 50,
            ramp_ms: 3_000,
        })
        .collect();
    let loss = GilbertElliott::new(0.01, 0.15, 0.01, 0.3);
    let a = build_path(&uplink, duration_ms, 40, loss, &handovers, seed)?;
    let b = build_path(&uplink, duration_ms, 45, loss, &[], seed.wrapping_add(1))?;
    Ok(Scenario {
        name: "saturated_city",
        duration_ms,
        paths: vec![a, b],
        shared_tower: None,
    })
}
