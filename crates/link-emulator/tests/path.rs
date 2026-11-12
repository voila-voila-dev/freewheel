//! Paths: both directions, data metering and radio power.

mod support;

use link_emulator::*;
use support::*;

fn idle_path(power: RadioPower) -> Path {
    let up = config(constant(1), 1_000_000, GilbertElliott::lossless(), vec![]);
    let down = config(constant(1), 1_000_000, GilbertElliott::lossless(), vec![]);
    Path::new(
        up,
        down,
        &[Event::Outage {
            start_ms: 2_000,
            end_ms: 2_500,
        }],
        power,
        1,
    )
}

#[test]
fn path_events_apply_to_both_directions() {
    let mut path = idle_path(RadioPower::lte_typical());
    let mut ups = Vec::new();
    let mut downs = Vec::new();
    for now in 0..3_000 {
        let (up, down) = path.advance(now, None);
        ups.extend(up);
        downs.extend(down);
        path.send_up(now, PACKET, now);
        path.send_down(now, 100, now);
    }
    let in_outage = |d: &Departure| (2_000..2_500).contains(&d.served_ms);
    assert!(!ups.iter().any(in_outage));
    assert!(!downs.iter().any(in_outage));
}

#[test]
fn data_meter_counts_every_transmitted_byte_both_ways() {
    let mut path = idle_path(RadioPower::lte_typical());
    for now in 0..1_000 {
        path.advance(now, None);
        if now < 100 {
            path.send_up(now, 1_000, now);
            path.send_down(now, 100, now);
        }
    }
    let meter = path.meter();
    assert_eq!(meter.up_bytes, 100_000);
    assert_eq!(meter.down_bytes, 10_000);
    // 110 kB at 10 €/GB = 0.11 cent, rounded up to 1 cent.
    assert_eq!(meter.cost_cents(1_000), 1);
}

#[test]
fn radio_power_follows_active_tail_and_idle_states() {
    let power = RadioPower {
        idle_mw: 10,
        active_mw: 1_000,
        tail_mw: 500,
        tail_ms: 1_000,
    };
    let mut path = idle_path(power);
    // One packet at t=0, then silence for 5 s.
    path.advance(0, None);
    path.send_up(0, 100, 0);
    path.advance(5_000, None);
    // ~1 active ms, ~1 s of tail at 500 mW, ~4 s idle at 10 mW → ~0.5 J + 0.04 J.
    let mj = path.energy_mj();
    assert!((530..=560).contains(&mj), "energy {mj} mJ");
}
