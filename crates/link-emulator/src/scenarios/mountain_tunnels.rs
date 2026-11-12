//! Mountain road with tunnels: hard outages and a new address after each.

use super::build_path::build_path;
use super::minute::MINUTE;
use super::scenario::Scenario;
use crate::{Event, GilbertElliott, RateModel, TraceError};

/// Mountain road with tunnels: good service broken by hard outages, an address change after each.
///
/// # Errors
/// Fails only if a generated trace is empty.
pub fn mountain_tunnels(seed: u64) -> Result<Scenario, TraceError> {
    let duration_ms = 30 * MINUTE;
    let uplink = RateModel::markov(500, &[2.0, 5.0, 8.0], 15_000);
    let mut events = Vec::new();
    for (start, length) in [(3, 45_000), (9, 120_000), (17, 30_000), (24, 90_000)] {
        let start_ms = start * MINUTE;
        events.push(Event::Outage {
            start_ms,
            end_ms: start_ms + length,
        });
        events.push(Event::AddressChange {
            at_ms: start_ms + length,
        });
    }
    let sim = build_path(
        &uplink,
        duration_ms,
        50,
        GilbertElliott::new(0.003, 0.2, 0.002, 0.3),
        &events,
        seed,
    )?;
    Ok(Scenario {
        name: "mountain_tunnels",
        duration_ms,
        paths: vec![sim],
        shared_tower: None,
    })
}
