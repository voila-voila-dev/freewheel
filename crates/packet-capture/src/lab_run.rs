//! Everything one lab scenario produced: both captures, its marks and the SRT statistics.

use std::path::Path;

use crate::{LabRunError, Mark, PcapFile, SrtStatistics, SrtlaReport, UdpDatagram};

/// Port the receiver (upstream `srtla_rec` or ours) listens on in the lab.
pub const LAB_RECEIVER_PORT: u16 = 5000;
const MICROSECONDS_PER_MILLISECOND: i64 = 1_000;
const SENDER_EXIT_PREFIX: &str = "sender ";
const SENDER_EXIT_INFIX: &str = " exited with ";

/// Everything one lab scenario produced.
#[derive(Debug, Clone)]
pub struct LabRun {
    /// Scenario name.
    pub scenario: String,
    /// What the box sent and received, before the network.
    pub box_side: SrtlaReport,
    /// What the server received and sent, after the network.
    pub server_side: SrtlaReport,
    /// What the scenario did, and when.
    pub marks: Vec<Mark>,
    /// What the SRT listener received (zero when it never ran).
    pub srt: SrtStatistics,
}

impl LabRun {
    /// Loads `<directory>/<scenario>.{box.pcap,server.pcap,marks,srt-stats.json}`.
    ///
    /// # Errors
    /// A capture or the marks file is missing or unreadable.
    pub fn load(directory: &Path, scenario: &str) -> Result<Self, LabRunError> {
        let file = |suffix: &str| directory.join(format!("{scenario}.{suffix}"));
        let marks = Mark::parse_all(&read_text(&file("marks"))?);
        let srt = read_text(&file("srt-stats.json")).map_or_else(
            |_| SrtStatistics::default(),
            |text| SrtStatistics::parse(&text),
        );
        Ok(Self {
            scenario: scenario.to_owned(),
            box_side: report_of(&file("box.pcap"))?,
            server_side: report_of(&file("server.pcap"))?,
            marks,
            srt,
        })
    }

    /// Time of the first mark whose label starts with `prefix`, in milliseconds since the
    /// box capture's first packet (negative if before it).
    #[must_use]
    pub fn mark_ms(&self, prefix: &str) -> Option<i64> {
        let mark = self
            .marks
            .iter()
            .find(|mark| mark.label.starts_with(prefix))?;
        Some(self.since_start_ms(mark))
    }

    /// When a mark happened, in milliseconds since the box capture's first packet.
    #[must_use]
    pub fn since_start_ms(&self, mark: &Mark) -> i64 {
        to_i64(mark.at_us).saturating_sub(to_i64(self.box_side.start_us))
            / MICROSECONDS_PER_MILLISECOND
    }

    /// Exit codes of the senders that stopped by themselves or were stopped, as `(sender, code)`.
    #[must_use]
    pub fn sender_exits(&self) -> Vec<(u32, i32)> {
        self.marks
            .iter()
            .filter_map(|mark| {
                let rest = mark.label.strip_prefix(SENDER_EXIT_PREFIX)?;
                let (sender, code) = rest.split_once(SENDER_EXIT_INFIX)?;
                Some((sender.parse().ok()?, code.parse().ok()?))
            })
            .collect()
    }
}

fn read_text(path: &Path) -> Result<String, LabRunError> {
    std::fs::read_to_string(path).map_err(|error| LabRunError::Unreadable {
        path: path.display().to_string(),
        error,
    })
}

fn report_of(path: &Path) -> Result<SrtlaReport, LabRunError> {
    let bytes = std::fs::read(path).map_err(|error| LabRunError::Unreadable {
        path: path.display().to_string(),
        error,
    })?;
    let capture_error = |error| LabRunError::Capture {
        path: path.display().to_string(),
        error,
    };
    let frames = PcapFile::parse(&bytes)
        .and_then(|file| file.frames())
        .map_err(capture_error)?;
    let datagrams: Vec<UdpDatagram<'_>> =
        frames.into_iter().filter_map(UdpDatagram::decode).collect();
    Ok(SrtlaReport::build(LAB_RECEIVER_PORT, &datagrams))
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
