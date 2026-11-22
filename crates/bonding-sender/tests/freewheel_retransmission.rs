//! Deadline-aware retransmission on a simulated network: a lost live packet is sent again
//! on the link where it arrives first, while it can still arrive in time, and reaches the
//! SRT listener once.

mod freewheel_network;

use freewheel_network::{LinkSpec, Network};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;
const MBPS: u64 = 1_000_000;

fn missing(network: &Network) -> Vec<u32> {
    (0..network.sent())
        .filter(|number| !network.delivered().contains(number))
        .collect()
}

fn duplicates(network: &Network) -> usize {
    let mut delivered = network.delivered().to_vec();
    let total = delivered.len();
    delivered.sort_unstable();
    delivered.dedup();
    total - delivered.len()
}

#[test]
fn random_loss_is_repaired() {
    let mut network = Network::new(&[
        LinkSpec::new(20 * MS, 10 * MBPS),
        LinkSpec::new(40 * MS, 10 * MBPS),
    ]);
    network.run(500 * MS, 0);
    network.set_lose_every(0, 20);
    network.set_lose_every(1, 20);
    network.run(3 * S, 4 * MBPS);
    network.run(500 * MS, 0);
    assert!(network.retransmitted() > 0);
    assert_eq!(missing(&network), Vec::<u32>::new());
    assert_eq!(
        duplicates(&network),
        0,
        "the listener gets each packet once"
    );
}

#[test]
fn most_of_a_dead_links_flight_goes_out_again_on_the_others() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 10 * MBPS),
        LinkSpec::new(30 * MS, 10 * MBPS),
    ]);
    network.run(500 * MS, 0);
    network.run(2 * S, 4 * MBPS);
    network.set_up(1, false);
    network.run(2 * S, 4 * MBPS);
    network.run(500 * MS, 0);
    assert!(!network.usable(1));
    // Most of it: the other link's window fills as it takes the whole stream, and a
    // retransmission only rides on spare capacity.
    let missing = missing(&network).len() as u64;
    assert!(
        network.retransmitted() > 2 * missing,
        "{} sent again, {missing} missing",
        network.retransmitted()
    );
}

#[test]
fn too_late_is_not_sent_again() {
    // 700 ms one way: a loss is seen 1.4 s after the first sending, and another 700 ms
    // would end past the deadline.
    let mut network = Network::new(&[LinkSpec::new(700 * MS, 10 * MBPS)]);
    network.run(3 * S, 0);
    network.set_lose_every(0, 10);
    network.run(3 * S, MBPS);
    network.run(2 * S, 0);
    assert_eq!(network.retransmitted(), 0);
    assert!(!missing(&network).is_empty());
}
