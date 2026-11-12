//! Synthetic traces fitted on, or written instead of, real traces.

use link_emulator::RateModel;

#[test]
fn fitted_synthetic_trace_keeps_the_statistics_without_copying() {
    let original = RateModel::markov(100, &[1.0, 4.0, 8.0], 2_000)
        .generate(120_000, 5)
        .unwrap_or_else(|e| unreachable!("{e}"));
    let model = RateModel::fit(&original, 100, 3);
    let synthetic = model
        .generate(120_000, 6)
        .unwrap_or_else(|e| unreachable!("{e}"));
    assert_ne!(synthetic, original);
    let (a, b) = (original.mean_bps(), synthetic.mean_bps());
    let gap = a.abs_diff(b) * 100 / a;
    assert!(gap < 20, "mean {a} vs synthetic {b} ({gap}% apart)");
}

#[test]
fn synthetic_traces_are_deterministic() {
    let model = RateModel::markov(200, &[1.0, 5.0], 1_000);
    assert_eq!(model.generate(10_000, 1), model.generate(10_000, 1));
}
