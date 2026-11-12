//! Parsing and looping Mahimahi traces.

mod support;

use link_emulator::*;
use support::*;

#[test]
fn parses_mahimahi_traces_and_loops() {
    let trace = Trace::parse("1\n1\n3\n\n5\n").unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(trace.len(), 4);
    assert_eq!(trace.period_ms(), 5);
    assert_eq!(trace.opportunity_ms(4), 6);
    assert_eq!(trace.opportunity_ms(7), 10);
}

#[test]
fn rejects_bad_traces() {
    assert_eq!(Trace::parse(""), Err(TraceError::Empty));
    assert_eq!(
        Trace::parse("1\nx\n"),
        Err(TraceError::InvalidLine { line: 2 })
    );
    assert_eq!(
        Trace::parse("5\n3\n"),
        Err(TraceError::NotSorted { line: 2 })
    );
}

#[test]
fn mean_capacity_matches_the_trace() {
    assert_eq!(constant(1).mean_bps(), 12_000_000);
}
