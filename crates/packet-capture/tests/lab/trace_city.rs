//! `trace-city`: two recorded LTE uplinks with room to spare (5 and 8–14 Mbit/s). Upstream hides
//! them: the video arrives whole, SRT's buffer is the only latency.

use crate::support::load;

/// SRT's receive buffer in the lab (`SRT_LATENCY_MS`).
const SRT_LATENCY_MS: f64 = 2_000.0;

#[test]
fn a_good_lte_network_is_hidden() {
    let Some(run) = load("trace-city") else {
        return;
    };
    let Some(frames) = run.frames else { return };
    assert!(frames.damaged() <= 10, "{frames:?}");
    if let Some(latency) = run.latency {
        assert!(latency.median_ms < SRT_LATENCY_MS + 200.0, "{latency:?}");
        assert!(latency.p99_ms < SRT_LATENCY_MS + 400.0, "{latency:?}");
    }
}
