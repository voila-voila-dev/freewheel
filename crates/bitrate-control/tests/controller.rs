//! The bitrate follows the links: down fast under pressure, up slowly once calm.

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
fn keeps_falling_by_a_fifth_per_window_while_pressure_lasts() {
    let mut controller = started();
    assert_eq!(
        feed(&mut controller, 4_000, 4_600, strained(20_000_000)),
        2_400_000
    );
    assert_eq!(
        feed(&mut controller, 4_600, 5_200, strained(20_000_000)),
        1_920_000
    );
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
fn no_usable_link_goes_to_the_minimum_even_at_start() {
    let mut controller = Controller::new(ControllerSettings::lab(MAX_BPS, 128_000));
    let report = CapacityReport {
        usable_links: 0,
        ..calm(0)
    };
    assert_eq!(feed(&mut controller, 0, 600, report), 300_000);
}

#[test]
fn climbs_back_fast_while_the_capacity_fits_more() {
    let mut controller = started();
    let low = feed(&mut controller, 4_000, 4_600, strained(500_000));
    let mut now_ms = 4_600;
    while controller.bitrate_bps() < MAX_BPS {
        let before = controller.bitrate_bps();
        let bps = controller.on_report(now_ms, &calm(10_000_000));
        assert!(bps <= before + before * 15 / 100 + 1);
        now_ms += 100;
    }
    // From 0.3 to 3 Mbit/s at 15 % a window (~600 ms with reports every 100 ms), after 2 s of
    // calm: about 12 s.
    assert!(
        low < 400_000 && (8_000..18_000).contains(&now_ms),
        "{low} {now_ms}"
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
