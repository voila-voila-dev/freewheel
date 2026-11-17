//! `steady`: two equal links share the traffic and nothing is lost.

use crate::support::{load, modem, share_percent};

#[test]
fn equal_links_share_the_traffic() {
    let Some(run) = load("steady") else { return };
    for k in [1, 2] {
        let share = share_percent(&run, modem(k), 2, 28);
        assert!((35..=65).contains(&share), "link {k}: {share} %");
    }
    assert_eq!(run.srt.dropped_packets, 0);
    assert_eq!(run.server_side.group_ids.len(), 1);
}

/// SRT's receive buffer in the lab (`SRT_LATENCY_MS`): every frame waits that long.
const SRT_LATENCY_MS: f64 = 2_000.0;

#[test]
fn latency_is_srt_buffer_and_little_more() {
    let Some(run) = load("steady") else { return };
    let Some(latency) = run.latency else {
        // Runs made before frame-times existed have no frame times.
        return;
    };
    assert!(latency.frames > 800, "{latency:?}");
    assert!(latency.median_ms >= SRT_LATENCY_MS, "{latency:?}");
    assert!(latency.median_ms < SRT_LATENCY_MS + 150.0, "{latency:?}");
    assert!(latency.p99_ms < SRT_LATENCY_MS + 300.0, "{latency:?}");
}
