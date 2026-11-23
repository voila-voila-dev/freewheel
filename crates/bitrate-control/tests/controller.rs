//! The bitrate follows the links: down fast under pressure, back up to the measured capacity
//! once calm, slowly beyond it.

use bitrate_control::{CapacityReport, Controller, ControllerSettings};

const MAX_BPS: u32 = 3_000_000;

fn calm(capacity_bps: u64) -> CapacityReport {
    CapacityReport {
        capacity_bps,
        sent_bps: 3_500_000,
        overflow_bps: 0,
        dropped_bps: 0,
        usable_links: 2,
    }
}

fn strained(capacity_bps: u64) -> CapacityReport {
    CapacityReport {
        dropped_bps: 500_000,
        ..calm(capacity_bps)
    }
}

/// Reports every 100 ms from `from_ms` until before `to_ms`; the last bitrate.
fn feed(controller: &mut Controller, from_ms: u64, to_ms: u64, report: CapacityReport) -> u32 {
    (from_ms..to_ms)
        .step_by(100)
        .map(|now_ms| controller.on_report(now_ms, &report))
        .last()
        .unwrap_or(controller.bitrate_bps())
}

/// A controller past its start-up grace, at its initial bitrate.
fn started() -> Controller {
    let mut controller = Controller::new(ControllerSettings::lab(MAX_BPS, 128_000));
    assert_eq!(feed(&mut controller, 0, 4_000, calm(8_000_000)), MAX_BPS);
    controller
}

#[test]
fn starts_at_its_initial_bitrate_and_stays_there_while_calm() {
    let mut controller = started();
    assert_eq!(
        feed(&mut controller, 4_000, 20_000, calm(8_000_000)),
        MAX_BPS
    );
}

#[test]
fn ignores_the_burst_of_the_first_seconds() {
    let mut controller = Controller::new(ControllerSettings::lab(MAX_BPS, 128_000));
    assert_eq!(
        feed(&mut controller, 0, 2_900, strained(1_000_000)),
        MAX_BPS
    );
    assert!(feed(&mut controller, 2_900, 3_600, strained(1_000_000)) < MAX_BPS);
}

#[test]
fn falls_to_what_the_links_carry_within_one_window() {
    let mut controller = started();
    let bps = feed(&mut controller, 4_000, 4_600, strained(2_000_000));
    // 2 Mbit/s × 80 % / 1.14 − 128 kbit/s.
    assert!((1_250_000..=1_300_000).contains(&bps), "{bps}");
}

#[test]
fn falls_by_at_least_a_fifth_then_holds_once_the_stream_fits() {
    let mut controller = started();
    // 4 Mbit/s fits 2.68 Mbit/s of video; the first decrease takes a fifth.
    assert_eq!(
        feed(&mut controller, 4_000, 4_600, strained(4_000_000)),
        2_400_000
    );
    // The stream fits: the pressure that lasts is not the encoder's.
    assert_eq!(
        feed(&mut controller, 4_600, 8_000, strained(4_000_000)),
        2_400_000
    );
    // Less capacity: down again.
    assert!(feed(&mut controller, 8_000, 8_600, strained(2_000_000)) < 1_300_000);
}

#[test]
fn a_cut_of_some_links_holds_what_the_others_carry_and_climbs_back_when_they_return() {
    let mut controller = started();
    // Two of three links cut: the third carries 2 Mbit/s, and SRT's catch-up keeps it full.
    let cut = CapacityReport {
        usable_links: 1,
        overflow_bps: 3_000_000,
        ..strained(2_000_000)
    };
    let held = feed(&mut controller, 4_000, 4_600, cut);
    assert!((1_250_000..=1_300_000).contains(&held), "{held}");
    assert_eq!(feed(&mut controller, 4_600, 8_000, cut), held);
    // The first report with the links back is enough.
    let back = CapacityReport {
        usable_links: 3,
        ..calm(8_000_000)
    };
    assert_eq!(controller.on_report(8_000, &back), MAX_BPS);
    assert_eq!(feed(&mut controller, 8_100, 12_000, back), MAX_BPS);
}

#[test]
fn a_keyframe_overflowing_for_a_moment_is_not_pressure_but_queuing_is() {
    let mut controller = started();
    let burst = CapacityReport {
        overflow_bps: 3_000_000,
        ..calm(8_000_000)
    };
    controller.on_report(4_000, &burst);
    assert_eq!(
        feed(&mut controller, 4_100, 6_000, calm(8_000_000)),
        MAX_BPS
    );
    let queuing = CapacityReport {
        overflow_bps: 1_000_000,
        ..calm(2_000_000)
    };
    assert!(feed(&mut controller, 6_000, 6_600, queuing) < MAX_BPS);
}

#[test]
fn an_outage_of_every_link_holds_the_bitrate() {
    let mut controller = started();
    let outage = CapacityReport {
        usable_links: 0,
        ..calm(0)
    };
    assert_eq!(feed(&mut controller, 4_000, 10_000, outage), MAX_BPS);
    assert_eq!(
        feed(&mut controller, 10_000, 12_000, calm(10_000_000)),
        MAX_BPS
    );
}

#[test]
fn climbs_straight_to_what_the_capacity_fits_once_calm() {
    let mut controller = started();
    let low = feed(&mut controller, 4_000, 4_600, strained(500_000));
    assert!(low < 400_000, "{low}");
    // 2 s of calm, then one step to what 2.5 Mbit/s carries: 2.5 × 80 % / 1.14 − 128 kbit/s.
    assert!(feed(&mut controller, 4_600, 6_500, calm(2_500_000)) < 400_000);
    let bps = feed(&mut controller, 6_500, 7_200, calm(2_500_000));
    assert!((1_550_000..=1_650_000).contains(&bps), "{bps}");
    assert_eq!(
        feed(&mut controller, 7_200, 8_000, calm(10_000_000)),
        MAX_BPS
    );
}

#[test]
fn after_an_outage_climbs_back_as_soon_as_the_links_return() {
    let mut controller = started();
    let low = feed(&mut controller, 4_000, 4_600, strained(500_000));
    assert!(low < 400_000, "{low}");
    let outage = CapacityReport {
        usable_links: 0,
        ..calm(0)
    };
    assert_eq!(feed(&mut controller, 4_600, 10_000, outage), low);
    // The links come back at the capacity they had: straight to it, at the first report.
    assert_eq!(controller.on_report(10_000, &calm(10_000_000)), MAX_BPS);
}

#[test]
fn needs_the_capacity_its_maximum_fits_in_with_headroom() {
    let settings = ControllerSettings::lab(MAX_BPS, 128_000);
    // (3 Mbit/s + 128 kbit/s) × 1.14 on the wire...
    assert_eq!(settings.wire_bps(MAX_BPS), 3_565_920);
    // ...in 80 % of the capacity.
    assert_eq!(settings.capacity_for_bps(MAX_BPS), 4_457_400);
    let mut controller = started();
    feed(&mut controller, 4_000, 4_600, strained(500_000));
    assert_eq!(
        feed(&mut controller, 4_600, 8_000, calm(4_457_400)),
        MAX_BPS
    );
}

#[test]
fn climbs_back_slowly_once_calm() {
    let mut controller = started();
    let low = feed(&mut controller, 4_000, 4_600, strained(1_000_000));
    // The window holding the last strained reports is judged at 4.6 s: one more decrease.
    let lowest = feed(&mut controller, 4_600, 6_500, calm(500_000));
    assert!(lowest <= low, "not calm yet: {lowest} after {low}");
    let mut now_ms = 6_500;
    let mut previous = lowest;
    // The links only measure what they are given: probing goes past what they report.
    while controller.bitrate_bps() < MAX_BPS {
        let bps = controller.on_report(now_ms, &calm(500_000));
        assert!(bps >= previous && bps <= previous + previous / 20 + 1);
        previous = bps;
        now_ms += 100;
    }
    // From 0.3 to 3 Mbit/s at 5 % a window (~600 ms): about 30 s.
    assert!((25_000..40_000).contains(&now_ms), "{now_ms}");
}

#[test]
fn reports_round_trip_through_json() {
    let report = CapacityReport {
        capacity_bps: 4_200_000,
        sent_bps: 3_500_000,
        overflow_bps: 12,
        dropped_bps: u64::MAX,
        usable_links: 3,
    };
    let mut out = [0u8; bitrate_control::MAX_REPORT_LEN];
    let len = report.encode(&mut out).unwrap_or_default();
    assert_eq!(CapacityReport::decode(&out[..len]), Some(report));
    assert_eq!(
        CapacityReport::decode(
            br#"{"capacity_bps":1,"sent_bps":2,"overflow_bps":3,"dropped_bps":4,"usable_links":5}"#
        ),
        Some(CapacityReport {
            capacity_bps: 1,
            sent_bps: 2,
            overflow_bps: 3,
            dropped_bps: 4,
            usable_links: 5
        })
    );
    for bad in [
        &b"{}"[..],
        br#"{"capacity_bps":1}"#,
        br#"{"capacity_bps":-1,"sent_bps":2,"overflow_bps":3,"dropped_bps":4,"usable_links":5}"#,
        br#"{"capacity_bps":1,"sent_bps":2,"overflow_bps":3,"dropped_bps":4,"usable_links":300}"#,
        br#"{"nope":1,"sent_bps":2,"overflow_bps":3,"dropped_bps":4,"usable_links":5}"#,
    ] {
        assert_eq!(CapacityReport::decode(bad), None);
    }
    assert_eq!(report.encode(&mut [0u8; 10]), None);
}
