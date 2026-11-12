//! Loading lab runs and measuring them.

use std::net::Ipv4Addr;
use std::path::PathBuf;

use packet_capture::{LabRun, LinkStats, SrtlaReport};

const CAPTURES_DIR: &str = "../../.cache/srtla-captures";

/// Loads a scenario's run. `None` (test skipped) when the lab has not been run, unless
/// `FREEWHEEL_REQUIRE_CAPTURES` is set, as in CI.
pub fn load(scenario: &str) -> Option<LabRun> {
    let directory = std::env::var_os("FREEWHEEL_CAPTURES_DIR")
        .map_or_else(|| PathBuf::from(CAPTURES_DIR), PathBuf::from);
    let run = LabRun::load(&directory, scenario);
    let required = std::env::var_os("FREEWHEEL_REQUIRE_CAPTURES").is_some();
    assert!(
        run.is_ok() || !required,
        "{scenario}: {:?} (run labs/srtla-capture/run-all.sh)",
        run.as_ref().err()
    );
    if run.is_err() {
        eprintln!("skipping {scenario}: run labs/srtla-capture/run-all.sh first");
    }
    run.ok()
}

/// The box's address on modem `k`.
pub fn modem(k: u8) -> Ipv4Addr {
    Ipv4Addr::new(10, k, 0, 2)
}

/// Every port a modem address used (a restarted sender uses new ports).
pub fn links_of(report: &SrtlaReport, address: Ipv4Addr) -> impl Iterator<Item = &LinkStats> {
    report
        .links
        .iter()
        .filter(move |(link, _)| *link.ip() == address)
        .map(|(_, stats)| stats)
}

/// Data packets the box sent on a modem during seconds `[from_s, to_s)` of the capture.
pub fn data_packets(run: &LabRun, address: Ipv4Addr, from_s: usize, to_s: usize) -> u64 {
    links_of(&run.box_side, address)
        .map(|stats| {
            stats
                .data_packets_per_second
                .iter()
                .skip(from_s)
                .take(to_s.saturating_sub(from_s))
                .sum::<u64>()
        })
        .sum()
}

/// A modem's share of all data packets during seconds `[from_s, to_s)`, in per cent.
pub fn share_percent(run: &LabRun, address: Ipv4Addr, from_s: usize, to_s: usize) -> u64 {
    let total: u64 = run
        .box_side
        .links
        .keys()
        .map(|link| *link.ip())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|other| data_packets(run, other, from_s, to_s))
        .sum();
    (data_packets(run, address, from_s, to_s) * 100)
        .checked_div(total)
        .unwrap_or(0)
}

/// Second of the capture at which a mark happened.
pub fn mark_s(run: &LabRun, prefix: &str) -> usize {
    let at_ms = run.mark_ms(prefix);
    assert!(at_ms.is_some(), "{}: no mark {prefix}", run.scenario);
    usize::try_from(at_ms.unwrap_or_default().max(0) / 1_000).unwrap_or_default()
}

/// Control packets of one raw type the box received on a modem.
pub fn received(run: &LabRun, address: Ipv4Addr, kind: u16) -> u64 {
    links_of(&run.box_side, address)
        .filter_map(|stats| stats.downlink_control.get(&kind))
        .sum()
}

/// Control packets of one raw type the box sent on a modem.
pub fn sent(run: &LabRun, address: Ipv4Addr, kind: u16) -> u64 {
    links_of(&run.box_side, address)
        .filter_map(|stats| stats.uplink_control.get(&kind))
        .sum()
}
