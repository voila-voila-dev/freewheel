//! Summarises `labs/srtla-capture/bench-transport.sh`: what each transport delivers, scenario by
//! scenario, on the measures M1b is judged on (`docs/technical/protocol/freewheel.md`, "How we
//! decide").
//!
//! Usage: `bench-transport <bench directory> [--json <file>] [--gate <receiver>-<sender>]`
//!
//! The directory holds `<receiver>-<sender>/run-<n>/` lab runs. Every measure is the median over
//! the runs: they all vary from run to run by themselves. `--gate` compares one combination (a
//! candidate) with the baseline, upstream SRTLA + SRT, and fails unless it does at least as well
//! on every scenario and loses fewer frames overall, and, on every scenario where it uploaded a
//! recording (M1b step 6), delivered all of it in every run.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use packet_capture::{LabRun, LossCauses, median, spread};

const USAGE: &str =
    "usage: bench-transport <bench directory> [--json <file>] [--gate <receiver>-<sender>]";
const BASELINE: &str = "upstream-upstream";
/// The scenario where one link dies and comes back alone: the only one whose restored link
/// share means something (in `all-links-drop` every link comes back at once).
const RESTORED_LINK_SCENARIO: &str = "link-drop";
/// Its two links are alike, so a restored link's fair share is half the traffic...
const RESTORED_LINK_EVEN_SHARE_PERCENT: f64 = 50.0;
/// ...and it must be back to at least this much of it. Not "at least irlserver's": irlserver
/// overshoots (up to 77 % of the traffic on the link that came back), which is no better.
const RESTORED_LINK_MIN_OF_EVEN: f64 = 0.9;
const MARKS_SUFFIX: &str = ".marks";

/// Frames lost on one scenario may exceed the baseline's by this share, or by
/// [`DAMAGED_SLACK_FRAMES`], or by the spread measured between runs of that scenario (the larger
/// of the two combinations' ranges), whichever is more: when the links cannot carry the stream
/// (`real-ride`, two of three links cut), both lose the same seconds and the count is a coin
/// toss between runs (49 to 63 frames over four CI runs for either, 2026-11-18). Stable
/// scenarios keep the tight bound. The total over every scenario must still be lower.
const DAMAGED_SLACK_RATIO: f64 = 0.05;
const DAMAGED_SLACK_FRAMES: f64 = 3.0;
/// Intact video may be this much below the baseline's: the encoder's own variation. Judged on
/// the intact frames, not on the bytes delivered: those count damaged frames too (`trace-drive`:
/// the baseline delivers 1.8 Mbit/s while two thirds of its frames are damaged), and an encoder
/// that lowers its bitrate to lose fewer frames delivers fewer bytes by design. The bytes
/// delivered stay in the table, for information.
const INTACT_SLACK_RATIO: f64 = 0.98;
/// Latency may be this much above the baseline's: scheduler noise of a shared host.
const MEDIAN_LATENCY_SLACK_MS: f64 = 20.0;
const TAIL_LATENCY_SLACK_MS: f64 = 50.0;

/// One scenario of one combination: medians over its runs, `NaN` when no run measured it.
#[derive(Debug, Clone, Copy)]
struct Summary {
    runs: usize,
    damaged_frames: f64,
    /// Largest minus smallest count of frames lost over the runs: the scenario's noise.
    damaged_spread_frames: f64,
    longest_gap_ms: f64,
    delivered_kbps: f64,
    intact_kbps: f64,
    latency_median_ms: f64,
    latency_p99_ms: f64,
    restored_share_percent: f64,
    /// Why frames were lost (`NaN` fields when the runs have no `netem` marks).
    causes: CauseSummary,
    /// The recording: share delivered identical in the worst run, how many runs recorded and
    /// how many got it whole, and the median time to whole after the stream ended.
    recording: RecordingSummary,
}

/// The recording upload over the runs of one scenario (`NaN` / 0 when none recorded).
#[derive(Debug, Clone, Copy)]
struct RecordingSummary {
    worst_delivered_percent: f64,
    runs: usize,
    whole_runs: usize,
    whole_after_s: f64,
}

/// Medians over the runs of the frames each cause explains ([`LossCauses`]).
#[derive(Debug, Clone, Copy)]
struct CauseSummary {
    capacity: f64,
    recovery: f64,
    isolated_loss: f64,
    link_down: f64,
    queue: f64,
    late: f64,
    short_seconds: f64,
}

/// One run of one scenario.
#[derive(Debug, Clone, Copy, Default)]
struct Sample {
    damaged_frames: Option<f64>,
    longest_gap_ms: Option<f64>,
    delivered_kbps: Option<f64>,
    intact_kbps: Option<f64>,
    latency_median_ms: Option<f64>,
    latency_p99_ms: Option<f64>,
    restored_share_percent: Option<f64>,
    causes: Option<LossCauses>,
    /// Share of the recording delivered identical, in per cent, if the run recorded.
    recording_delivered_percent: Option<f64>,
    recording_whole: bool,
    recording_whole_after_s: Option<f64>,
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
    print!("{}", causes_table(&results));
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
    let recorded = results
        .iter()
        .filter(|((_, combination), s)| combination == candidate && s.recording.runs > 0)
        .count();
    if recorded == 0 {
        println!("\n{candidate} uploaded no recording: its gate is on the live stream only.");
    } else {
        println!(
            "\n{candidate} uploaded a recording on {recorded} scenarios: each must arrive whole in every run."
        );
    }
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
        intact_kbps: lab_run.intact_kbps(),
        latency_median_ms: lab_run.latency.map(|latency| latency.median_ms),
        latency_p99_ms: lab_run.latency.map(|latency| latency.p99_ms),
        restored_share_percent: (scenario == RESTORED_LINK_SCENARIO)
            .then(|| lab_run.restored_link_share_percent())
            .flatten(),
        causes: LossCauses::load(run, scenario).ok().flatten(),
        recording_delivered_percent: lab_run
            .recording
            .and_then(|recording| recording.delivered_percent()),
        recording_whole: lab_run.recording.is_some_and(|recording| recording.whole()),
        recording_whole_after_s: lab_run.recording_whole_after_s(),
    }
}

fn summary(samples: &[Sample]) -> Summary {
    let median_of = |pick: fn(&Sample) -> Option<f64>| median(samples.iter().filter_map(pick));
    Summary {
        runs: samples.len(),
        damaged_frames: median_of(|s| s.damaged_frames),
        damaged_spread_frames: spread(samples.iter().filter_map(|s| s.damaged_frames)),
        longest_gap_ms: median_of(|s| s.longest_gap_ms),
        delivered_kbps: median_of(|s| s.delivered_kbps),
        intact_kbps: median_of(|s| s.intact_kbps),
        latency_median_ms: median_of(|s| s.latency_median_ms),
        latency_p99_ms: median_of(|s| s.latency_p99_ms),
        restored_share_percent: median_of(|s| s.restored_share_percent),
        causes: cause_summary(samples),
        recording: RecordingSummary {
            worst_delivered_percent: samples
                .iter()
                .filter_map(|s| s.recording_delivered_percent)
                .reduce(f64::min)
                .unwrap_or(f64::NAN),
            runs: samples
                .iter()
                .filter(|s| s.recording_delivered_percent.is_some())
                .count(),
            whole_runs: samples.iter().filter(|s| s.recording_whole).count(),
            whole_after_s: median_of(|s| s.recording_whole_after_s),
        },
    }
}

fn cause_summary(samples: &[Sample]) -> CauseSummary {
    let median_of = |pick: fn(&LossCauses) -> usize| {
        median(
            samples
                .iter()
                .filter_map(|s| s.causes.as_ref())
                .map(|causes| to_f64(pick(causes))),
        )
    };
    CauseSummary {
        capacity: median_of(|c| c.capacity),
        recovery: median_of(|c| c.recovery),
        isolated_loss: median_of(|c| c.isolated_loss),
        link_down: median_of(|c| c.link_down),
        queue: median_of(|c| c.queue),
        late: median_of(|c| c.late),
        short_seconds: median_of(|c| c.short_seconds),
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
        "| Scenario | Sender → receiver | Runs | Frames lost or damaged | Longest gap | Intact video | Delivered | Latency p50 / p99 | Restored link share | Recording delivered |"
    );
    let _ = writeln!(text, "|---|---|---|---|---|---|---|---|---|---|");
    for ((scenario, combination), s) in results {
        let _ = writeln!(
            text,
            "| {scenario} | {} | {} | {} | {} ms | {} kbit/s | {} kbit/s | {} / {} ms | {} | {} |",
            label(combination),
            s.runs,
            show(s.damaged_frames, 0),
            show(s.longest_gap_ms, 0),
            show(s.intact_kbps, 0),
            show(s.delivered_kbps, 0),
            show(s.latency_median_ms, 0),
            show(s.latency_p99_ms, 0),
            show_percent(s.restored_share_percent),
            show_recording(&s.recording),
        );
    }
    text
}

/// "100 % in 12 s" when every run got the whole recording (median time after the stream
/// ended), else the worst run's share and how many runs were whole; "–" without a recording.
fn show_recording(recording: &RecordingSummary) -> String {
    if recording.runs == 0 {
        return "–".to_owned();
    }
    if recording.whole_runs == recording.runs {
        return format!("100 % in {} s", show(recording.whole_after_s, 0));
    }
    format!(
        "{:.1} % ({}/{} runs whole)",
        recording.worst_delivered_percent, recording.whole_runs, recording.runs
    )
}

/// Why frames were lost, per scenario (`packet_capture::LossCauses`): only scenarios that lost
/// frames, and runs with `netem` marks.
fn causes_table(results: &Results) -> String {
    let mut text = String::from(
        "\nWhy frames were lost (median of the runs): capacity = the links together carried less \
         than the stream (that second, the one before or after); recovery = 2–5 s after such a second, \
         nothing lost on the way (catching up); isolated = lost on a lossy link \
         while capacity sufficed; link down = sent into a cut or dead link; queue = a full queue; \
         late = every packet arrived, too late.\n\n\
         | Scenario | Sender → receiver | Short seconds | Capacity | Recovery | Isolated loss | Link down | Queue | Late |\n\
         |---|---|---|---|---|---|---|---|---|\n",
    );
    let mut any = false;
    for ((scenario, combination), s) in results {
        let c = s.causes;
        if c.capacity.is_nan() || s.damaged_frames == 0.0 {
            continue;
        }
        any = true;
        let _ = writeln!(
            text,
            "| {scenario} | {} | {} | {} | {} | {} | {} | {} | {} |",
            label(combination),
            show(c.short_seconds, 0),
            show(c.capacity, 0),
            show(c.recovery, 0),
            show(c.isolated_loss, 0),
            show(c.link_down, 0),
            show(c.queue, 0),
            show(c.late, 0),
        );
    }
    if any { text } else { String::new() }
}

fn show(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        "–".to_owned()
    } else {
        format!("{value:.decimals$}")
    }
}

fn show_percent(value: f64) -> String {
    if value.is_nan() {
        "–".to_owned()
    } else {
        format!("{value:.0} %")
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
                ours.damaged_frames
                    <= baseline.damaged_frames
                        + (baseline.damaged_frames * DAMAGED_SLACK_RATIO)
                            .max(DAMAGED_SLACK_FRAMES)
                            .max(ours.damaged_spread_frames)
                            .max(baseline.damaged_spread_frames),
            ),
            (
                "intact video (kbit/s)",
                ours.intact_kbps,
                baseline.intact_kbps,
                ours.intact_kbps >= baseline.intact_kbps * INTACT_SLACK_RATIO,
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
        // A restored link's share (`link-drop`): close to an even split, and the baseline's.
        let restored_check = (!ours.restored_share_percent.is_nan()).then(|| {
            let even_bar = RESTORED_LINK_EVEN_SHARE_PERCENT * RESTORED_LINK_MIN_OF_EVEN;
            let bar = if baseline.restored_share_percent.is_nan() {
                even_bar
            } else {
                even_bar.max(baseline.restored_share_percent)
            };
            (
                "restored link share (%)",
                ours.restored_share_percent,
                bar,
                ours.restored_share_percent >= bar,
            )
        });
        // The recording, where the candidate uploaded one: all of it, in every run. The
        // baseline has none, so the bar is absolute.
        if ours.recording.runs > 0 && ours.recording.whole_runs < ours.recording.runs {
            failures.push(format!(
                "{scenario}, recording delivered: {}",
                show_recording(&ours.recording)
            ));
        }
        for (what, ours, baseline, holds) in checks.into_iter().chain(restored_check) {
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
            ("intact_kbps", s.intact_kbps),
            ("latency_median_ms", s.latency_median_ms),
            ("latency_p99_ms", s.latency_p99_ms),
            ("restored_share_percent", s.restored_share_percent),
            (
                "recording_worst_delivered_percent",
                s.recording.worst_delivered_percent,
            ),
            ("recording_whole_after_s", s.recording.whole_after_s),
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
