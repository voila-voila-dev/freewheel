//! Summarises `labs/srtla-capture/bench.sh` and checks that ours is at least on par with upstream.
//!
//! Usage: `bench-srtla <bench directory> [--json <file>]`
//!
//! The directory holds `<receiver>-<sender>/run-<n>/` lab runs. Other work on the host only ever
//! adds CPU and memory and takes away delivered bytes, so those are the best run (least CPU and
//! memory, most delivered): the least noisy estimate. Recovery after a dead link is the median,
//! because it varies from run to run by itself. The gate compares our sender with upstream's (both against upstream's receiver) and
//! our receiver with upstream's (both behind upstream's sender); irlserver is reported only.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::ExitCode;

use packet_capture::LabRun;

const USAGE: &str = "usage: bench-srtla <bench directory> [--json <file>]";
const UPSTREAM: &str = "upstream";
const FREEWHEEL: &str = "freewheel";
/// Restored link's share, measured 15–25 s after it came back.
const RESTORED_SHARE_FROM_S: usize = 15;
const RESTORED_SHARE_TO_S: usize = 25;

/// Delivered share may be this many points below upstream's.
const DELIVERED_SLACK_POINTS: f64 = 2.0;
/// CPU may be this much above upstream's: 10 %, plus one scheduler tick of noise per second.
const CPU_SLACK_RATIO: f64 = 1.10;
const CPU_SLACK_MS: f64 = 30.0;
/// Peak memory may be this much above upstream's.
const MEMORY_SLACK_RATIO: f64 = 1.25;
const MEMORY_SLACK_KB: f64 = 1_024.0;
/// Recovery after a dead link: gap, damaged frames and restored share.
const GAP_SLACK_MS: f64 = 200.0;
const DAMAGED_SLACK_FRAMES: f64 = 10.0;
const SHARE_SLACK_POINTS: f64 = 10.0;

/// One combination's summary: best run for CPU, memory and delivery, median for recovery. `NaN`
/// when no run has the measurement.
#[derive(Debug, Clone, Copy)]
struct Medians {
    runs: usize,
    throughput_delivered_percent: f64,
    throughput_dropped_packets: f64,
    throughput_sender_cpu_ms: f64,
    throughput_receiver_cpu_ms: f64,
    aggregation_delivered_percent: f64,
    steady_sender_cpu_ms: f64,
    steady_receiver_cpu_ms: f64,
    sender_peak_kb: f64,
    receiver_peak_kb: f64,
    link_drop_damaged_frames: f64,
    link_drop_gap_ms: f64,
    link_drop_restored_share_percent: f64,
}

/// One lab run's measurements, before taking medians.
#[derive(Debug, Clone, Copy, Default)]
struct Sample {
    throughput_delivered_percent: Option<f64>,
    throughput_dropped_packets: Option<f64>,
    throughput_sender_cpu_ms: Option<f64>,
    throughput_receiver_cpu_ms: Option<f64>,
    aggregation_delivered_percent: Option<f64>,
    steady_sender_cpu_ms: Option<f64>,
    steady_receiver_cpu_ms: Option<f64>,
    sender_peak_kb: Option<f64>,
    receiver_peak_kb: Option<f64>,
    link_drop_damaged_frames: Option<f64>,
    link_drop_gap_ms: Option<f64>,
    link_drop_restored_share_percent: Option<f64>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (directory, json) = match arguments.as_slice() {
        [directory] => (directory, None),
        [directory, flag, json] if flag == "--json" => (directory, Some(json)),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let combinations = match measure(Path::new(directory)) {
        Ok(combinations) if !combinations.is_empty() => combinations,
        Ok(_) => {
            eprintln!("{directory}: no runs (labs/srtla-capture/bench.sh)");
            return ExitCode::FAILURE;
        }
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    print!("{}", table(&combinations));
    if let Some(path) = json
        && let Err(error) = std::fs::write(path, to_json(&combinations))
    {
        eprintln!("cannot write {path}: {error}");
        return ExitCode::FAILURE;
    }
    let failures = gate(&combinations);
    if failures.is_empty() {
        println!("\nOn par with upstream: every gate holds.");
        ExitCode::SUCCESS
    } else {
        println!("\nBehind upstream:");
        for failure in &failures {
            println!("- {failure}");
        }
        ExitCode::FAILURE
    }
}

/// `<receiver>-<sender>` → medians over its runs.
fn measure(directory: &Path) -> Result<BTreeMap<String, Medians>, String> {
    let mut combinations = BTreeMap::new();
    for combination in sorted_directories(directory)? {
        let mut samples = Vec::new();
        for run in sorted_directories(&directory.join(&combination))? {
            samples.push(sample(&directory.join(&combination).join(run)));
        }
        if !samples.is_empty() {
            combinations.insert(combination, medians(&samples));
        }
    }
    Ok(combinations)
}

fn sorted_directories(directory: &Path) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name != "runs")
        .collect();
    names.sort();
    Ok(names)
}

fn sample(run: &Path) -> Sample {
    let mut sample = Sample::default();
    if let Ok(throughput) = LabRun::load(run, "throughput") {
        sample.throughput_delivered_percent = delivered_percent(&throughput);
        sample.throughput_dropped_packets = Some(to_f64(throughput.srt.dropped_packets));
        sample.throughput_sender_cpu_ms = cpu_ms(&throughput, "sender");
        sample.throughput_receiver_cpu_ms = cpu_ms(&throughput, "receiver");
        sample.sender_peak_kb = peak_kb(&throughput, "sender");
        sample.receiver_peak_kb = peak_kb(&throughput, "receiver");
    }
    if let Ok(aggregation) = LabRun::load(run, "aggregation") {
        sample.aggregation_delivered_percent = delivered_percent(&aggregation);
    }
    if let Ok(steady) = LabRun::load(run, "steady") {
        sample.steady_sender_cpu_ms = cpu_ms(&steady, "sender");
        sample.steady_receiver_cpu_ms = cpu_ms(&steady, "receiver");
    }
    if let Ok(link_drop) = LabRun::load(run, "link-drop") {
        if let Some(frames) = link_drop.frames {
            sample.link_drop_damaged_frames = Some(to_f64(frames.damaged()));
            sample.link_drop_gap_ms = Some(to_f64(frames.longest_gap_ms));
        }
        sample.link_drop_restored_share_percent = restored_share_percent(&link_drop);
    }
    sample
}

fn delivered_percent(run: &LabRun) -> Option<f64> {
    let sent = run.raw_stream_bytes().filter(|bytes| *bytes > 0)?;
    Some(to_f64(run.received_bytes) * 100.0 / to_f64(sent))
}

fn cpu_ms(run: &LabRun, role: &str) -> Option<f64> {
    run.resources(role)
        .map(|resources| to_f64(resources.cpu_ms))
}

fn peak_kb(run: &LabRun, role: &str) -> Option<f64> {
    run.resources(role)
        .map(|resources| to_f64(resources.peak_kb))
}

/// Link 2's share of data packets 15–25 s after `restore link 2`, in per cent.
fn restored_share_percent(run: &LabRun) -> Option<f64> {
    let restore_s = usize::try_from(run.mark_ms("restore link 2")?.max(0) / 1_000).ok()?;
    let window = (
        restore_s + RESTORED_SHARE_FROM_S,
        restore_s + RESTORED_SHARE_TO_S,
    );
    let link_2 = Ipv4Addr::new(10, 2, 0, 2);
    let mut on_link_2 = 0_u64;
    let mut total = 0_u64;
    for (link, stats) in &run.box_side.links {
        let packets: u64 = stats
            .data_packets_per_second
            .iter()
            .skip(window.0)
            .take(window.1 - window.0)
            .sum();
        total += packets;
        if *link.ip() == link_2 {
            on_link_2 += packets;
        }
    }
    (total > 0).then(|| to_f64(on_link_2) * 100.0 / to_f64(total))
}

fn medians(samples: &[Sample]) -> Medians {
    let median_of = |pick: fn(&Sample) -> Option<f64>| median(samples.iter().filter_map(pick));
    let least_of = |pick: fn(&Sample) -> Option<f64>| {
        samples
            .iter()
            .filter_map(pick)
            .reduce(f64::min)
            .unwrap_or(f64::NAN)
    };
    let most_of = |pick: fn(&Sample) -> Option<f64>| {
        samples
            .iter()
            .filter_map(pick)
            .reduce(f64::max)
            .unwrap_or(f64::NAN)
    };
    Medians {
        runs: samples.len(),
        throughput_delivered_percent: most_of(|s| s.throughput_delivered_percent),
        throughput_dropped_packets: least_of(|s| s.throughput_dropped_packets),
        throughput_sender_cpu_ms: least_of(|s| s.throughput_sender_cpu_ms),
        throughput_receiver_cpu_ms: least_of(|s| s.throughput_receiver_cpu_ms),
        aggregation_delivered_percent: most_of(|s| s.aggregation_delivered_percent),
        steady_sender_cpu_ms: least_of(|s| s.steady_sender_cpu_ms),
        steady_receiver_cpu_ms: least_of(|s| s.steady_receiver_cpu_ms),
        sender_peak_kb: least_of(|s| s.sender_peak_kb),
        receiver_peak_kb: least_of(|s| s.receiver_peak_kb),
        link_drop_damaged_frames: median_of(|s| s.link_drop_damaged_frames),
        link_drop_gap_ms: median_of(|s| s.link_drop_gap_ms),
        link_drop_restored_share_percent: median_of(|s| s.link_drop_restored_share_percent),
    }
}

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut values: Vec<f64> = values.collect();
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        values[middle]
    } else {
        f64::midpoint(values[middle - 1], values[middle])
    }
}

/// `<receiver>-<sender>` → "sender → receiver".
fn label(combination: &str) -> String {
    combination.split_once('-').map_or_else(
        || combination.to_owned(),
        |(receiver, sender)| format!("{sender} → {receiver}"),
    )
}

fn table(combinations: &BTreeMap<String, Medians>) -> String {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "| Sender → receiver | Runs | 50 Mbit/s delivered | dropped | CPU sender / receiver (50 Mbit/s, 20 s) | CPU sender / receiver (3 Mbit/s video, 30 s) | Peak memory sender / receiver | 12 Mbit/s over 2 × 8 delivered | Dead link: damaged frames, gap | Restored link share 15–25 s after |"
    );
    let _ = writeln!(text, "|---|---|---|---|---|---|---|---|---|---|");
    for (combination, m) in combinations {
        let _ = writeln!(
            text,
            "| {} | {} | {} % | {} | {} / {} ms | {} / {} ms | {} / {} kB | {} % | {}, {} ms | {} % |",
            label(combination),
            m.runs,
            show(m.throughput_delivered_percent, 1),
            show(m.throughput_dropped_packets, 0),
            show(m.throughput_sender_cpu_ms, 0),
            show(m.throughput_receiver_cpu_ms, 0),
            show(m.steady_sender_cpu_ms, 0),
            show(m.steady_receiver_cpu_ms, 0),
            show(m.sender_peak_kb, 0),
            show(m.receiver_peak_kb, 0),
            show(m.aggregation_delivered_percent, 1),
            show(m.link_drop_damaged_frames, 0),
            show(m.link_drop_gap_ms, 0),
            show(m.link_drop_restored_share_percent, 0),
        );
    }
    text
}

fn show(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        "–".to_owned()
    } else {
        format!("{value:.decimals$}")
    }
}

/// What ours misses against upstream, one line per gate.
fn gate(combinations: &BTreeMap<String, Medians>) -> Vec<String> {
    let reference = format!("{UPSTREAM}-{UPSTREAM}");
    let Some(upstream) = combinations.get(&reference) else {
        return vec![format!("no {reference} runs to compare with")];
    };
    let mut failures = Vec::new();
    for (combination, role) in [
        (format!("{UPSTREAM}-{FREEWHEEL}"), Role::Sender),
        (format!("{FREEWHEEL}-{UPSTREAM}"), Role::Receiver),
    ] {
        let Some(ours) = combinations.get(&combination) else {
            failures.push(format!("no {combination} runs"));
            continue;
        };
        for comparison in comparisons(ours, upstream, role) {
            if !comparison.holds() {
                failures.push(format!(
                    "our {}, {}: {} against upstream's {}",
                    role.name(),
                    comparison.what,
                    show(comparison.ours, 1),
                    show(comparison.upstream, 1)
                ));
            }
        }
    }
    failures
}

/// The side of the bonding under test.
#[derive(Debug, Clone, Copy)]
enum Role {
    Sender,
    Receiver,
}

impl Role {
    const fn name(self) -> &'static str {
        match self {
            Self::Sender => "sender",
            Self::Receiver => "receiver",
        }
    }

    /// CPU at 50 Mbit/s, CPU at 3 Mbit/s and peak memory of this side.
    const fn costs(self, medians: &Medians) -> (f64, f64, f64) {
        match self {
            Self::Sender => (
                medians.throughput_sender_cpu_ms,
                medians.steady_sender_cpu_ms,
                medians.sender_peak_kb,
            ),
            Self::Receiver => (
                medians.throughput_receiver_cpu_ms,
                medians.steady_receiver_cpu_ms,
                medians.receiver_peak_kb,
            ),
        }
    }
}

/// One gate: ours against upstream, and the bound ours must respect.
struct Comparison {
    what: &'static str,
    ours: f64,
    upstream: f64,
    bound: Bound,
}

enum Bound {
    AtLeast(f64),
    AtMost(f64),
}

impl Comparison {
    fn holds(&self) -> bool {
        match self.bound {
            Bound::AtLeast(minimum) => self.ours >= minimum,
            Bound::AtMost(maximum) => self.ours <= maximum,
        }
    }
}

fn comparisons(ours: &Medians, upstream: &Medians, role: Role) -> Vec<Comparison> {
    let at_least = |what, ours: f64, upstream: f64, slack: f64| Comparison {
        what,
        ours,
        upstream,
        bound: Bound::AtLeast(upstream - slack),
    };
    let at_most = |what, ours: f64, upstream: f64, maximum: f64| Comparison {
        what,
        ours,
        upstream,
        bound: Bound::AtMost(maximum),
    };
    let cpu_limit = |reference: f64| reference * CPU_SLACK_RATIO + CPU_SLACK_MS;
    let (cpu, steady_cpu, peak) = role.costs(ours);
    let (cpu_reference, steady_cpu_reference, peak_reference) = role.costs(upstream);
    vec![
        at_least(
            "50 Mbit/s delivered (%)",
            ours.throughput_delivered_percent,
            upstream.throughput_delivered_percent,
            DELIVERED_SLACK_POINTS,
        ),
        at_least(
            "aggregation delivered (%)",
            ours.aggregation_delivered_percent,
            upstream.aggregation_delivered_percent,
            DELIVERED_SLACK_POINTS,
        ),
        at_most(
            "CPU at 50 Mbit/s (ms)",
            cpu,
            cpu_reference,
            cpu_limit(cpu_reference),
        ),
        at_most(
            "CPU at 3 Mbit/s (ms)",
            steady_cpu,
            steady_cpu_reference,
            cpu_limit(steady_cpu_reference),
        ),
        at_most(
            "peak memory (kB)",
            peak,
            peak_reference,
            (peak_reference * MEMORY_SLACK_RATIO).max(peak_reference + MEMORY_SLACK_KB),
        ),
        at_most(
            "dead link, longest gap (ms)",
            ours.link_drop_gap_ms,
            upstream.link_drop_gap_ms,
            upstream.link_drop_gap_ms + GAP_SLACK_MS,
        ),
        at_most(
            "dead link, damaged frames",
            ours.link_drop_damaged_frames,
            upstream.link_drop_damaged_frames,
            upstream.link_drop_damaged_frames + DAMAGED_SLACK_FRAMES,
        ),
        at_least(
            "restored link share (%)",
            ours.link_drop_restored_share_percent,
            upstream.link_drop_restored_share_percent,
            SHARE_SLACK_POINTS,
        ),
    ]
}

fn to_json(combinations: &BTreeMap<String, Medians>) -> String {
    let mut text = String::from("{\n");
    let count = combinations.len();
    for (index, (combination, m)) in combinations.iter().enumerate() {
        let fields = [
            (
                "throughput_delivered_percent",
                m.throughput_delivered_percent,
            ),
            ("throughput_dropped_packets", m.throughput_dropped_packets),
            ("throughput_sender_cpu_ms", m.throughput_sender_cpu_ms),
            ("throughput_receiver_cpu_ms", m.throughput_receiver_cpu_ms),
            (
                "aggregation_delivered_percent",
                m.aggregation_delivered_percent,
            ),
            ("steady_sender_cpu_ms", m.steady_sender_cpu_ms),
            ("steady_receiver_cpu_ms", m.steady_receiver_cpu_ms),
            ("sender_peak_kb", m.sender_peak_kb),
            ("receiver_peak_kb", m.receiver_peak_kb),
            ("link_drop_damaged_frames", m.link_drop_damaged_frames),
            ("link_drop_gap_ms", m.link_drop_gap_ms),
            (
                "link_drop_restored_share_percent",
                m.link_drop_restored_share_percent,
            ),
        ];
        let _ = write!(text, "  \"{combination}\": {{ \"runs\": {}", m.runs);
        for (name, value) in fields {
            if value.is_nan() {
                let _ = write!(text, ", \"{name}\": null");
            } else {
                let _ = write!(text, ", \"{name}\": {value:.3}");
            }
        }
        let separator = if index + 1 == count { "" } else { "," };
        let _ = writeln!(text, " }}{separator}");
    }
    text.push_str("}\n");
    text
}

#[allow(clippy::cast_precision_loss)]
fn to_f64<T: TryInto<u64>>(value: T) -> f64 {
    value.try_into().map_or(f64::NAN, |value: u64| value as f64)
}
