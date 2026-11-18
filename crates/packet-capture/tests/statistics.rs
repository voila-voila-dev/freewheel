//! Order statistics over a handful of runs.

use packet_capture::{median, spread};

#[test]
fn the_spread_is_how_far_apart_the_runs_landed() {
    assert!((spread([56.0, 49.0, 62.0]) - 13.0).abs() < f64::EPSILON);
    assert!(spread([7.0]).abs() < f64::EPSILON, "one run: no spread");
    assert!(spread([]).is_nan(), "no run: unknown");
}

#[test]
fn the_median_of_an_even_count_is_the_midpoint() {
    assert!((median([1.0, 3.0, 2.0, 10.0]) - 2.5).abs() < f64::EPSILON);
    assert!(median([]).is_nan());
}
