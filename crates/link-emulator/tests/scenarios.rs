//! Ready-made days on the road.

mod support;

use link_emulator::{Tower, scenarios};
use support::PACKET;

#[test]
fn every_scenario_builds_and_runs() {
    let scenarios = scenarios::all(1).unwrap_or_else(|e| unreachable!("{e}"));
    let names: Vec<_> = scenarios.iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        [
            "desert_day",
            "saturated_city",
            "mountain_tunnels",
            "shared_tower"
        ]
    );
    for mut scenario in scenarios {
        let mut tower = scenario
            .shared_tower
            .map(|congestion| Tower::new(congestion, 1));
        for now in (0..scenario.duration_ms.min(120_000)).step_by(10) {
            for (index, path) in scenario.paths.iter_mut().enumerate() {
                path.advance(now, tower.as_mut());
                path.send_up(now * 10 + index as u64, PACKET, now);
            }
        }
        assert!(
            scenario.paths.iter().all(|p| p.meter().up_bytes > 0),
            "{} carried data",
            scenario.name
        );
    }
}
