//! Replays public cellular traces, when they have been downloaded with `bun run traces:fetch`.
//! Skipped (with a note) otherwise, so offline runs still pass.

use link_emulator::{RateModel, Trace};
use std::path::PathBuf;

fn traces_dir() -> PathBuf {
    std::env::var_os("FREEWHEEL_TRACES_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/traces/mahimahi"),
        PathBuf::from,
    )
}

fn load_all() -> Vec<(String, Trace)> {
    let Ok(entries) = std::fs::read_dir(traces_dir()) else {
        return Vec::new();
    };
    let mut traces: Vec<(String, Trace)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(entry.path()).ok()?;
            Some((name, Trace::parse(&text).ok()?))
        })
        .collect();
    traces.sort_by(|a, b| a.0.cmp(&b.0));
    traces
}

#[test]
fn public_traces_parse_and_synthesize_with_the_same_mean_capacity() {
    let traces = load_all();
    if traces.is_empty() {
        eprintln!("public traces not downloaded: run `bun run traces:fetch` to enable this test");
        return;
    }
    for (name, original) in &traces {
        let model = RateModel::fit(original, 100, 8);
        let synthetic = model
            .generate(original.period_ms(), 1)
            .unwrap_or_else(|error| unreachable!("{name}: {error}"));
        assert_ne!(
            &synthetic, original,
            "{name}: synthetic must not copy the original"
        );
        let (real, fake) = (original.mean_bps(), synthetic.mean_bps());
        let gap = real.abs_diff(fake) * 100 / real.max(1);
        assert!(
            gap < 25,
            "{name}: mean {real} bps vs synthetic {fake} bps ({gap}% apart)"
        );
    }
}
