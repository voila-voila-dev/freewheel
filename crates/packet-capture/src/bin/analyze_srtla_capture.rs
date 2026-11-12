//! Prints what the lab captured of upstream SRTLA.
//!
//! Usage:
//! - `analyze-srtla-capture <directory> <scenario>`: both sides of one scenario, with its marks
//! - `analyze-srtla-capture <directory>`: one summary line per scenario found
//! - `analyze-srtla-capture <capture.pcap>`: one capture

use std::path::Path;
use std::process::ExitCode;

use packet_capture::{LAB_RECEIVER_PORT, LabRun, PcapFile, SrtlaReport, UdpDatagram};

const BOX_CAPTURE_SUFFIX: &str = ".box.pcap";
const USAGE: &str = "usage: analyze-srtla-capture <directory> [scenario] | <capture.pcap>";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let result = match arguments.as_slice() {
        [file]
            if Path::new(file)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("pcap")) =>
        {
            print_capture(Path::new(file))
        }
        [directory] => print_summary(Path::new(directory)),
        [directory, scenario] => print_scenario(Path::new(directory), scenario),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn print_capture(path: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let frames = PcapFile::parse(&bytes)
        .and_then(|file| file.frames())
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
    let datagrams: Vec<UdpDatagram<'_>> =
        frames.into_iter().filter_map(UdpDatagram::decode).collect();
    println!(
        "# {}\n\n{} UDP datagrams\n",
        path.display(),
        datagrams.len()
    );
    print!("{}", SrtlaReport::build(LAB_RECEIVER_PORT, &datagrams));
    Ok(())
}

fn print_scenario(directory: &Path, scenario: &str) -> Result<(), String> {
    let run = LabRun::load(directory, scenario).map_err(|error| error.to_string())?;
    println!("# {scenario}\n\n## Marks (ms since the first box packet)");
    for mark in &run.marks {
        println!("- {:>7}  {}", run.since_start_ms(mark), mark.label);
    }
    println!("\n## SRT listener\n{:?}\n", run.srt);
    println!("# Box side\n\n{}", run.box_side);
    println!("# Server side\n\n{}", run.server_side);
    Ok(())
}

fn print_summary(directory: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    let mut scenarios: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(BOX_CAPTURE_SUFFIX).map(str::to_owned))
        .collect();
    scenarios.sort();
    println!(
        "| Scenario | Data packets per link (box side) | Groups | SRT received / dropped | Sender exits |"
    );
    println!("|---|---|---|---|---|");
    for scenario in scenarios {
        match LabRun::load(directory, &scenario) {
            Ok(run) => {
                let mut per_link: Vec<String> = run
                    .box_side
                    .links
                    .iter()
                    .filter(|(_, stats)| stats.data_packets > 0)
                    .map(|(link, stats)| format!("{}: {}", link.ip(), stats.data_packets))
                    .collect();
                let silent = run.box_side.links.len() - per_link.len();
                if silent > 0 {
                    per_link.push(format!("+{silent} without data"));
                }
                println!(
                    "| {scenario} | {} | {} | {} / {} | {:?} |",
                    per_link.join(", "),
                    run.server_side.group_ids.len(),
                    run.srt.received_packets,
                    run.srt.dropped_packets,
                    run.sender_exits()
                );
            }
            Err(error) => println!("| {scenario} | {error} | | | |"),
        }
    }
    Ok(())
}
