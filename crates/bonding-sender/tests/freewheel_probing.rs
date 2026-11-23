//! Capacity probing: padding on spare capacity tells the encoder what the links carry, not
//! only what they were given, without costing the live stream.

mod freewheel_network;

use bitrate_control::ControllerSettings;
use bonding_sender::freewheel::{PROBE_BUDGET_PERCENT, ProbeTarget};
use freewheel_network::{LinkSpec, Network};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;
const MBPS: u64 = 1_000_000;
/// The lab's encoder: 3 Mbit/s of video at most, 128 kbit/s of audio.
const MAX_VIDEO_BPS: u32 = 3_000_000;

fn lab_target() -> ProbeTarget {
    let settings = ControllerSettings::lab(MAX_VIDEO_BPS, 128_000);
    ProbeTarget {
        stream_max_wire_bps: settings.wire_bps(MAX_VIDEO_BPS),
        wanted_capacity_bps: settings.capacity_for_bps(MAX_VIDEO_BPS),
    }
}

fn delivered_all(network: &Network) -> bool {
    network.delivered().len() == usize::try_from(network.sent()).unwrap_or(0)
}

#[test]
fn learns_what_spare_links_carry_while_the_encoder_is_low() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 8 * MBPS),
        LinkSpec::new(50 * MS, 8 * MBPS),
    ]);
    network.enable_probing(lab_target());
    network.run(500 * MS, 0);
    network.run(6 * S, 400_000);
    network.run(300 * MS, 0);
    let report = network.last_report();
    assert!(
        report.capacity_bps >= lab_target().wanted_capacity_bps,
        "the links carry 16 Mbit/s, the encoder's maximum needs {}: {report:?}",
        lab_target().wanted_capacity_bps
    );
    assert!(delivered_all(&network), "padding cost live packets");
}

#[test]
fn padding_stays_within_its_budget() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 2 * MBPS),
        LinkSpec::new(50 * MS, 2 * MBPS),
    ]);
    network.enable_probing(lab_target());
    network.run(500 * MS, 0);
    network.run(20 * S, 400_000);
    // The links carry 4 Mbit/s, less than the encoder's maximum needs: probing never stops.
    let padding_bps = network.sender.padding_bytes() * 8 / 20;
    assert!(
        padding_bps * 100 <= 4 * MBPS * PROBE_BUDGET_PERCENT,
        "{padding_bps} bit/s of padding"
    );
    assert!(padding_bps > 0, "never probed");
}

#[test]
fn does_not_probe_when_the_encoder_is_at_its_maximum() {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 8 * MBPS),
        LinkSpec::new(50 * MS, 8 * MBPS),
    ]);
    network.enable_probing(lab_target());
    network.run(500 * MS, 0);
    network.run(5 * S, 3_300_000);
    assert_eq!(network.sender.padding_bytes(), 0);
}

#[test]
fn a_link_back_from_an_outage_is_known_at_its_former_rate_within_a_second() {
    let mut network = Network::new(&[LinkSpec::new(20 * MS, 8 * MBPS)]);
    network.enable_probing(lab_target());
    network.run(500 * MS, 0);
    // Known at what the encoder's maximum needs, from a low stream.
    network.run(5 * S, 400_000);
    let before = network.last_report().capacity_bps;
    assert!(before >= lab_target().wanted_capacity_bps, "{before}");
    network.set_up(0, false);
    network.run(4 * S, 300_000);
    assert_eq!(network.last_report().usable_links, 0);
    network.set_up(0, true);
    network.run(S, 300_000);
    let after = network.last_report();
    assert!(
        after.capacity_bps * 10 >= before * 9,
        "{before} before the outage, {after:?} a second after"
    );
}

#[test]
fn probing_a_link_that_cannot_carry_more_does_not_delay_live_packets() {
    let mut network = Network::new(&[LinkSpec::new(30 * MS, MBPS)]);
    network.enable_probing(lab_target());
    network.run(500 * MS, 0);
    network.run(10 * S, 400_000);
    network.run(500 * MS, 0);
    let slowest_us = network.take_latencies_us().into_iter().max().unwrap_or(0);
    assert!(delivered_all(&network), "padding cost live packets");
    assert!(
        slowest_us < 300 * MS,
        "slowest live packet took {} ms",
        slowest_us / MS
    );
    assert!(network.sender.padding_bytes() > 0, "never probed");
}
