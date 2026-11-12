//! Long empty roads: one weak SIM, long outages between villages.

use super::build_path::build_path;
use super::minute::MINUTE;
use super::scenario::Scenario;
use crate::{Event, GilbertElliott, RateModel, TraceError};

/// Long empty roads: one weak SIM, low capacity, long outages between villages.
///
/// # Errors
/// Fails only if a generated trace is empty.
pub fn desert_day(seed: u64) -> Result<Scenario, TraceError> {
    let duration_ms = 60 * MINUTE;
    let uplink = RateModel::markov(500, &[0.3, 1.0, 2.5, 4.0], 20_000);
    let events = [
        Event::Outage {
            start_ms: 8 * MINUTE,
            end_ms: 20 * MINUTE,
        },
        Event::Outage {
            start_ms: 35 * MINUTE,
            end_ms: 37 * MINUTE,
        },
        Event::ModemReboot {
            at_ms: 50 * MINUTE,
            down_ms: 40_000,
        },
    ];
    let sim = build_path(
        &uplink,
        duration_ms,
        60,
        GilbertElliott::new(0.002, 0.1, 0.005, 0.4),
        &events,
        seed,
    )?;
    Ok(Scenario {
        name: "desert_day",
        duration_ms,
        paths: vec![sim],
        shared_tower: None,
    })
}
