//! The freewheel sender and receiver together, on a simulated network: joining, carrying
//! SRT both ways, sharing traffic by expected arrival, leaving and taking back a dead link.

mod freewheel_network;

use freewheel_network::{LinkSpec, Network};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;
const MBPS: u64 = 1_000_000;

fn delivered_share(network: &Network) -> f64 {
    f64::from(u32::try_from(network.delivered().len()).unwrap_or(0)) / f64::from(network.sent())
}

#[test]
fn joins_and_carries_srt_both_ways() {
    let mut network = Network::new(&[LinkSpec::new(20 * MS, 10 * MBPS)]);
    network.run(300 * MS, 0);
    assert!(network.usable(0), "the link joined");
    network.run(2 * S, 3 * MBPS);
    network.run(200 * MS, 0);
    let missing: Vec<u32> = (0..network.sent())
        .filter(|number| !network.delivered().contains(number))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
    network.srt_answer();
    network.run(100 * MS, 0);
    assert_eq!(network.to_caller(), 1);
}

#[test]
fn prefers_the_link_that_arrives_first() {
    let mut network = Network::new(&[
        LinkSpec::new(20 * MS, 10 * MBPS),
        LinkSpec::new(100 * MS, 10 * MBPS),
    ]);
    network.run(500 * MS, 0);
    network.run(2 * S, 2 * MBPS);
    network.take_data_counts();
    network.run(3 * S, 2 * MBPS);
    let counts = network.take_data_counts();
    assert!(counts[0] > 9 * counts[1], "{counts:?}");
}

#[test]
fn spreads_a_stream_no_single_link_can_carry() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 4 * MBPS),
        LinkSpec::new(50 * MS, 4 * MBPS),
    ]);
    network.run(500 * MS, 0);
    network.run(5 * S, 6 * MBPS);
    network.run(300 * MS, 0);
    let counts = network.take_data_counts();
    assert!(
        delivered_share(&network) > 0.98,
        "{}",
        delivered_share(&network)
    );
    assert!(counts.iter().all(|count| *count > 0), "{counts:?}");
}

#[test]
fn leaves_a_dead_link_and_takes_it_back_when_it_returns() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 3 * MBPS),
        LinkSpec::new(30 * MS, 3 * MBPS),
    ]);
    network.run(500 * MS, 0);
    network.run(3 * S, 4 * MBPS);
    network.set_up(1, false);
    network.run(600 * MS, 2 * MBPS);
    assert!(!network.usable(1), "the dead link stalled");
    network.take_data_counts();
    network.run(2 * S, 2 * MBPS);
    assert_eq!(network.take_data_counts()[1], 0);
    network.set_up(1, true);
    network.run(200 * MS, 4 * MBPS);
    assert!(network.usable(1), "back within 200 ms");
    network.take_data_counts();
    network.run(2 * S, 4 * MBPS);
    let counts = network.take_data_counts();
    assert!(
        counts[1] * 3 > counts[0],
        "the returned link takes its share back: {counts:?}"
    );
}

#[test]
fn moves_traffic_off_a_link_whose_capacity_collapses() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 4 * MBPS),
        LinkSpec::new(30 * MS, 4 * MBPS),
    ]);
    network.run(500 * MS, 0);
    network.run(3 * S, 3 * MBPS);
    network.set_rate(0, MBPS / 2);
    network.run(S, 3 * MBPS);
    network.take_data_counts();
    let before = network.delivered().len();
    network.run(3 * S, 3 * MBPS);
    let counts = network.take_data_counts();
    let delivered = network.delivered().len() - before;
    assert!(counts[1] > 4 * counts[0], "{counts:?}");
    assert!(
        50 * delivered as u64 > 49 * (counts[0] + counts[1]),
        "{delivered} of {counts:?}"
    );
}

#[test]
fn does_not_fill_a_modem_queue_with_video_that_would_arrive_too_late() {
    let mut network = Network::new(&[LinkSpec {
        delay_us: 40 * MS,
        rate_bps: 6 * MBPS,
        max_queue_us: 3 * S,
    }]);
    network.run(500 * MS, 0);
    network.run(2 * S, 3 * MBPS);
    network.set_rate(0, MBPS);
    network.run(5 * S, 3 * MBPS);
    network.set_rate(0, 6 * MBPS);
    network.run(500 * MS, 3 * MBPS);
    network.take_latencies_us();
    network.run(2 * S, 3 * MBPS);
    let latencies_us = network.take_latencies_us();
    let late = latencies_us.iter().filter(|latency| **latency > S).count();
    assert!(!latencies_us.is_empty());
    assert_eq!(late, 0, "{late} of {} packets over 1 s", latencies_us.len());
}
