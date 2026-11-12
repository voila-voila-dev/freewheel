//! End-to-end: a real SRT stream through 2–4 emulated links, checked frame by frame.
//!
//! Unlike the characterisation tests (`tests/lab/`), which describe what upstream does, these
//! are requirements: every combination must meet them, upstream ↔ upstream, upstream sender →
//! our receiver, our sender → upstream receiver and Rust ↔ Rust. Limits come from measured
//! runs of all four (two campaigns, 2026-11-12) with some slack, so they catch regressions, not noise.
//!
//! Counted once the stream has started (the first frames sent before the SRT connection is up
//! are reported apart, and bounded) and until it ends.

use std::path::PathBuf;

use packet_capture::{FrameCheck, LabRun};

const CAPTURES_DIR: &str = "../../.cache/srtla-captures";
/// The lab starts the encoder once SRT is connected: no frame may be sent before. Only
/// `cold-start` and `group-limit` start it early, on purpose, and they are not listed here.
const MAX_LATE_START_FRAMES: usize = 0;
/// What the SRT listener may still be holding when the lab stops everything.
const MAX_CUT_FRAMES: usize = 45;

/// What a scenario must deliver.
struct Limit {
    scenario: &'static str,
    /// Frames lost or damaged once started.
    max_damaged: usize,
    /// Longest run of frames not delivered intact, in milliseconds.
    max_gap_ms: i64,
}

const fn limit(scenario: &'static str, max_damaged: usize, max_gap_ms: i64) -> Limit {
    Limit {
        scenario,
        max_damaged,
        max_gap_ms,
    }
}

/// Healthy or impaired links that bonding and SRT fully hide. Over two campaigns of all four
/// combinations (2026-11-12) they lost 0–6 frames, at most 132 ms, upstream ↔ upstream
/// `steady` included: SRT dropping a late packet on a loaded host, not the bonding.
const CLEAN: (usize, i64) = (10, 250);

const LIMITS: &[Limit] = &[
    // 2 and 4 healthy links.
    limit("steady", CLEAN.0, CLEAN.1),
    limit("four-links", CLEAN.0, CLEAN.1),
    limit("asymmetric-links", CLEAN.0, CLEAN.1),
    limit("same-carrier", CLEAN.0, CLEAN.1),
    limit("low-bitrate", CLEAN.0, CLEAN.1),
    // Impaired links: loss, jitter, queues, varying capacity. Bonding and SRT hide it all.
    limit("random-loss", CLEAN.0, CLEAN.1),
    limit("burst-loss", CLEAN.0, CLEAN.1),
    limit("jitter-reordering", CLEAN.0, CLEAN.1),
    limit("bufferbloat", CLEAN.0, CLEAN.1),
    limit("latency-spike", CLEAN.0, CLEAN.1),
    limit("varying-bandwidth", CLEAN.0, CLEAN.1),
    // One link dying or flapping while the others carry on: a hiccup at most.
    limit("link-drop", 15, 300),
    limit("link-drop-long", 15, 300),
    limit("link-flapping", 20, 300),
    limit("modem-unplugged", 15, 400),
    limit("uplink-only-drop", CLEAN.0, CLEAN.1),
    limit("downlink-only-drop", CLEAN.0, CLEAN.1),
    limit("link-added", CLEAN.0, CLEAN.1),
    limit("link-removed", 10, 300),
    limit("address-change", CLEAN.0, CLEAN.1),
    limit("nat-rebinding", CLEAN.0, CLEAN.1),
    // Every link dead for 6 s: the gap is the tunnel plus at most 1.5 s, then it recovers.
    limit("all-links-drop", 240, 7_500),
    // Two minutes of everything at once.
    limit("real-ride", 200, 2_000),
    // Ten minutes: nothing accumulates.
    limit("long-run", 5, 100),
];

fn load(scenario: &str) -> Option<LabRun> {
    let directory = std::env::var_os("FREEWHEEL_CAPTURES_DIR")
        .map_or_else(|| PathBuf::from(CAPTURES_DIR), PathBuf::from);
    let run = LabRun::load(&directory, scenario);
    let required = std::env::var_os("FREEWHEEL_REQUIRE_CAPTURES").is_some();
    assert!(
        run.is_ok() || !required,
        "{scenario}: {:?} (run labs/srtla-capture/run-all.sh)",
        run.as_ref().err()
    );
    run.ok()
}

fn frames(scenario: &str) -> Option<FrameCheck> {
    let run = load(scenario)?;
    let required = std::env::var_os("FREEWHEEL_REQUIRE_CAPTURES").is_some();
    assert!(
        run.frames.is_some() || !required,
        "{scenario}: no frame digests"
    );
    run.frames
}

#[test]
fn every_scenario_delivers_its_frames() {
    let mut failures = Vec::new();
    for limit in LIMITS {
        let Some(check) = frames(limit.scenario) else {
            eprintln!(
                "skipping {}: run labs/srtla-capture/run-all.sh first",
                limit.scenario
            );
            continue;
        };
        eprintln!("{}: {check:?}", limit.scenario);
        if check.intact == 0 {
            failures.push(format!("{}: no frame arrived intact", limit.scenario));
            continue;
        }
        if check.damaged() > limit.max_damaged {
            failures.push(format!(
                "{}: {} frames damaged or lost, at most {}",
                limit.scenario,
                check.damaged(),
                limit.max_damaged
            ));
        }
        if check.longest_gap_ms > limit.max_gap_ms {
            failures.push(format!(
                "{}: {} ms without video, at most {}",
                limit.scenario, check.longest_gap_ms, limit.max_gap_ms
            ));
        }
        if check.before_start > MAX_LATE_START_FRAMES {
            failures.push(format!(
                "{}: started {} frames late, at most {MAX_LATE_START_FRAMES}",
                limit.scenario, check.before_start
            ));
        }
        if check.truncated > MAX_CUT_FRAMES {
            failures.push(format!(
                "{}: last {} frames never arrived, at most {MAX_CUT_FRAMES}",
                limit.scenario, check.truncated
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_dead_link_costs_at_most_a_hiccup() {
    let Some(check) = frames("link-drop") else {
        return;
    };
    assert!(check.intact_percent() > 98.0, "{check:?}");
}

#[test]
fn the_stream_comes_back_after_every_link_died() {
    let Some(check) = frames("all-links-drop") else {
        return;
    };
    // 35 s stream, 6 s tunnel: what follows the tunnel arrives.
    assert!(check.intact_percent() > 75.0, "{check:?}");
    assert!(check.truncated <= MAX_CUT_FRAMES, "{check:?}");
}
