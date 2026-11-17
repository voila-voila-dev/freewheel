//! Summarises `labs/srtla-capture/bench-transport.sh`: what each transport delivers, scenario by
//! scenario, on the measures M1b is judged on (`docs/technical/protocol/freewheel.md`, "How we
//! decide").
//!
//! Usage: `bench-transport <bench directory> [--json <file>] [--gate <receiver>-<sender>]`
//!
//! The directory holds `<receiver>-<sender>/run-<n>/` lab runs. Every measure is the median over
//! the runs: they all vary from run to run by themselves. `--gate` compares one combination (a
//! candidate) with the baseline, upstream SRTLA + SRT, and fails unless it does at least as well
//! on every scenario and loses fewer frames overall.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use packet_capture::{LabRun, median};

const USAGE: &str =
    "usage: bench-transport <bench directory> [--json <file>] [--gate <receiver>-<sender>]";
const BASELINE: &str = "upstream-upstream";
const MARKS_SUFFIX: &str = ".marks";

/// Delivered video may be this much below the baseline's: the encoder's own variation.
const DELIVERED_SLACK_RATIO: f64 = 0.98;
/// Latency may be this much above the baseline's: scheduler noise of a shared host.
const MEDIAN_LATENCY_SLACK_MS: f64 = 20.0;
const TAIL_LATENCY_SLACK_MS: f64 = 50.0;

/// One scenario of one combination: medians over its runs, `NaN` when no run measured it.
#[derive(Debug, Clone, Copy)]
struct Summary {
    runs: usize,
    damaged_frames: f64,
    longest_gap_ms: f64,
    delivered_kbps: f64,
    latency_median_ms: f64,
    latency_p99_ms: f64,
}

/// One run of one scenario.
#[derive(Debug, Clone, Copy, Default)]
struct Sample {
    damaged_frames: Option<f64>,
    longest_gap_ms: Option<f64>,
    delivered_kbps: Option<f64>,
    latency_median_ms: Option<f64>,
    latency_p99_ms: Option<f64>,
}

/// `(scenario, combination)` → summary.
type Results = BTreeMap<(String, String), Summary>;

struct Options {
    directory: String,
    json: Option<String>,
    gate: Option<String>,
}

fn main() -> ExitCode {
    let Some(options) = parse_options(std::env::args().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let results = match measure(Path::new(&options.directory)) {
        Ok(results) if !results.is_empty() => results,
        Ok(_) => {
            eprintln!(
                "{}: no runs (labs/srtla-capture/bench-transport.sh)",
                options.directory
            );
            return ExitCode::FAILURE;
        }
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    print!("{}", table(&results));
    if let Some(path) = &options.json
        && let Err(error) = std::fs::write(path, to_json(&results))
    {
        eprintln!("cannot write {path}: {error}");
        return ExitCode::FAILURE;
    }
    let Some(candidate) = &options.gate else {
        return ExitCode::SUCCESS;
    };
    let failures = gate(&results, candidate);
    println!(
        "\nRecording delivered after outages: not measured yet (needs the background traffic class)."
    );
    if failures.is_empty() {
        println!("{candidate} beats the baseline on every scenario.");
        ExitCode::SUCCESS
    } else {
        println!("{candidate} does not beat the baseline:");
        for failure in &failures {
            println!("- {failure}");
        }
        ExitCode::FAILURE
    }
}

fn parse_options(arguments: Vec<String>) -> Option<Options> {
    let mut arguments = arguments.into_iter();
    let mut options = Options {
        directory: arguments.next()?,
        json: None,
        gate: None,
    };
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--json" => options.json = Some(arguments.next()?),
            "--gate" => options.gate = Some(arguments.next()?),
            _ => return None,
        }
    }
    Some(options)
}

fn measure(directory: &Path) -> Result<Results, String> {
    let mut samples: BTreeMap<(String, String), Vec<Sample>> = BTreeMap::new();
    for combination in sorted_directories(directory)? {
        for run in sorted_directories(&directory.join(&combination))? {
            let run_directory = directory.join(&combination).join(run);
            for scenario in scenarios_in(&run_directory)? {
                samples
                    .entry((scenario.clone(), combination.clone()))
                    .or_default()
                    .push(sample(&run_directory, &scenario));
            }
        }
    }
    Ok(samples
        .into_iter()
        .map(|(key, samples)| (key, summary(&samples)))
        .collect())
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

/// Scenarios that ran in this directory: one `<scenario>.marks` each.
fn scenarios_in(directory: &Path) -> Result<BTreeSet<String>, String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    Ok(entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(MARKS_SUFFIX).map(str::to_owned))
        .collect())
}

fn sample(run: &Path, scenario: &str) -> Sample {
    let Ok(lab_run) = LabRun::load(run, scenario) else {
        return Sample::default();
    };
    Sample {
        damaged_frames: lab_run.frames.map(|frames| to_f64(frames.damaged())),
        longest_gap_ms: lab_run.frames.map(|frames| to_f64(frames.longest_gap_ms)),
        delivered_kbps: lab_run.delivered_kbps(),
        latency_median_ms: lab_run.latency.map(|latency| latency.median_ms),
        latency_p99_ms: lab_run.latency.map(|latency| latency.p99_ms),
    }
}

fn summary(samples: &[Sample]) -> Summary {
    let median_of = |pick: fn(&Sample) -> Option<f64>| median(samples.iter().filter_map(pick));
    Summary {
        runs: samples.len(),
        damaged_frames: median_of(|s| s.damaged_frames),
        longest_gap_ms: median_of(|s| s.longest_gap_ms),
        delivered_kbps: median_of(|s| s.delivered_kbps),
        latency_median_ms: median_of(|s| s.latency_median_ms),
        latency_p99_ms: median_of(|s| s.latency_p99_ms),
    }
}

/// `<receiver>-<sender>` → "sender → receiver".
fn label(combination: &str) -> String {
    combination.split_once('-').map_or_else(
        || combination.to_owned(),
        |(receiver, sender)| format!("{sender} → {receiver}"),
    )
}

fn table(results: &Results) -> String {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "| Scenario | Sender → receiver | Runs | Frames lost or damaged | Longest gap | Delivered | Latency p50 / p99 | Recording delivered |"
    );
    let _ = writeln!(text, "|---|---|---|---|---|---|---|---|");
    for ((scenario, combination), s) in results {
        let _ = writeln!(
            text,
            "| {scenario} | {} | {} | {} | {} ms | {} kbit/s | {} / {} ms | – |",
            label(combination),
            s.runs,
            show(s.damaged_frames, 0),
            show(s.longest_gap_ms, 0),
            show(s.delivered_kbps, 0),
            show(s.latency_median_ms, 0),
            show(s.latency_p99_ms, 0),
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

/// What the candidate misses against the baseline, one line per scenario and measure.
fn gate(results: &Results, candidate: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut candidate_damaged = 0.0;
    let mut baseline_damaged = 0.0;
    let scenarios: BTreeSet<&String> = results.keys().map(|(scenario, _)| scenario).collect();
    for scenario in scenarios {
        let get = |combination: &str| results.get(&(scenario.clone(), combination.to_owned()));
        let (Some(ours), Some(baseline)) = (get(candidate), get(BASELINE)) else {
            failures.push(format!("{scenario}: no runs of {candidate} or {BASELINE}"));
            continue;
        };
        candidate_damaged += ours.damaged_frames;
        baseline_damaged += baseline.damaged_frames;
        let checks = [
            (
                "frames lost or damaged",
                ours.damaged_frames,
                baseline.damaged_frames,
                ours.damaged_frames <= baseline.damaged_frames,
            ),
            (
                "delivered (kbit/s)",
                ours.delivered_kbps,
                baseline.delivered_kbps,
                ours.delivered_kbps >= baseline.delivered_kbps * DELIVERED_SLACK_RATIO,
            ),
            (
                "latency p50 (ms)",
                ours.latency_median_ms,
                baseline.latency_median_ms,
                ours.latency_median_ms <= baseline.latency_median_ms + MEDIAN_LATENCY_SLACK_MS,
            ),
            (
                "latency p99 (ms)",
                ours.latency_p99_ms,
                baseline.latency_p99_ms,
                ours.latency_p99_ms <= baseline.latency_p99_ms + TAIL_LATENCY_SLACK_MS,
            ),
        ];
        for (what, ours, baseline, holds) in checks {
            if !holds {
                failures.push(format!(
                    "{scenario}, {what}: {} against {}",
                    show(ours, 0),
                    show(baseline, 0)
                ));
            }
        }
    }
    if candidate_damaged.partial_cmp(&baseline_damaged) != Some(std::cmp::Ordering::Less) {
        failures.push(format!(
            "frames lost or damaged, all scenarios: {} against {} (must be fewer)",
            show(candidate_damaged, 0),
            show(baseline_damaged, 0)
        ));
    }
    failures
}

fn to_json(results: &Results) -> String {
    let mut text = String::from("[\n");
    let count = results.len();
    for (index, ((scenario, combination), s)) in results.iter().enumerate() {
        let fields = [
            ("damaged_frames", s.damaged_frames),
            ("longest_gap_ms", s.longest_gap_ms),
            ("delivered_kbps", s.delivered_kbps),
            ("latency_median_ms", s.latency_median_ms),
            ("latency_p99_ms", s.latency_p99_ms),
        ];
        let _ = write!(
            text,
            "  {{ \"scenario\": \"{scenario}\", \"combination\": \"{combination}\", \"runs\": {}",
            s.runs
        );
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
    text.push_str("]\n");
    text
}

#[allow(clippy::cast_precision_loss)]
fn to_f64<T: TryInto<u64>>(value: T) -> f64 {
    value.try_into().map_or(f64::NAN, |value: u64| value as f64)
}
