//! Everything one lab scenario produced: both captures, its marks and the SRT statistics.

use std::net::Ipv4Addr;
use std::path::Path;

use crate::{
    FrameCheck, FrameDigests, FrameLatency, FrameTime, LabRunError, Mark, PcapFile,
    ProcessResources, RecordingComparison, SrtStatistics, SrtlaReport, UdpDatagram,
};

/// Port the receiver (upstream `srtla_rec` or ours) listens on in the lab.
pub const LAB_RECEIVER_PORT: u16 = 5000;
const MICROSECONDS_PER_MILLISECOND: i64 = 1_000;
const SENDER_EXIT_PREFIX: &str = "sender ";
const SENDER_EXIT_INFIX: &str = " exited with ";
const RAW_STREAM_PREFIX: &str = "raw stream 1 started at ";
const BITS_PER_BYTE: u64 = 8;
const STREAM_PREFIX: &str = "stream 1 started at ";
const BITS_PER_KILOBIT: f64 = 1_000.0;
const MILLISECONDS_PER_SECOND: i64 = 1_000;
const MILLISECONDS_PER_SECOND_F64: f64 = 1_000.0;
/// The link `link-drop` cuts and restores.
const RESTORED_LINK: Ipv4Addr = Ipv4Addr::new(10, 2, 0, 2);
const RESTORED_MARK: &str = "restore link 2";
/// The restored link's share is measured this long after it came back, in seconds.
const RESTORED_SHARE_FROM_S: usize = 15;
const RESTORED_SHARE_TO_S: usize = 25;
/// Marks of stream 1's recording (`labs/srtla-capture/lib/recording.sh`): written (the stream's
/// end), then whole on the server.
const RECORDING_WRITTEN_MARK: &str = "recording 1 written ";
const RECORDING_DELIVERED_MARK: &str = "recording 1 delivered";

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
    /// The received video against what stream 1 sent, frame by frame (`None` when the run
    /// has no frame digests).
    pub frames: Option<FrameCheck>,
    /// Encoder-to-server latency of the intact frames of stream 1 (`None` when the run has no
    /// frame times: `<scenario>.{sent.1,received}.times`).
    pub latency: Option<FrameLatency>,
    /// Bytes the SRT listener wrote to `<scenario>.ts`: `<scenario>.received-bytes` when the
    /// video was discarded, else the file's size (0 when there is neither).
    pub received_bytes: u64,
    /// The server's copy of stream 1's recording against the recording
    /// (`<scenario>.recording`; `None` when the run recorded nothing).
    pub recording: Option<RecordingComparison>,
}

impl LabRun {
    /// Loads `<directory>/<scenario>.{box.pcap,server.pcap,marks,srt-stats.json}` and, when
    /// present, the frame digests `<scenario>.{sent.1,received}.frames`.
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
        let digests = match (
            read_text(&file("sent.1.frames")),
            read_text(&file("received.frames")),
        ) {
            (Ok(sent), Ok(received)) => {
                Some((FrameDigests::parse(&sent), FrameDigests::parse(&received)))
            }
            _ => None,
        };
        let frames = digests
            .as_ref()
            .map(|(sent, received)| FrameCheck::compare(sent, received));
        let latency = match (
            &digests,
            read_text(&file("sent.1.times")),
            read_text(&file("received.times")),
        ) {
            (Some((sent, received)), Ok(sent_times), Ok(received_times)) => FrameLatency::measure(
                &FrameTime::parse_all(&sent_times),
                &FrameTime::parse_all(&received_times),
                &sent.intact_in(received),
            ),
            _ => None,
        };
        Ok(Self {
            scenario: scenario.to_owned(),
            box_side: report_of(&file("box.pcap"))?,
            server_side: report_of(&file("server.pcap"))?,
            marks,
            srt,
            frames,
            latency,
            received_bytes: read_text(&file("received-bytes"))
                .ok()
                .and_then(|text| text.trim().parse().ok())
                .unwrap_or_else(|| {
                    std::fs::metadata(file("ts")).map_or(0, |metadata| metadata.len())
                }),
            recording: read_text(&file("recording"))
                .ok()
                .and_then(|text| RecordingComparison::parse(&text)),
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

    /// Link 2's share of the data packets 15–25 s after `restore link 2` (`link-drop`), in per
    /// cent: how much traffic a restored link got back. `None` without that mark or traffic.
    #[must_use]
    pub fn restored_link_share_percent(&self) -> Option<f64> {
        let restore_s =
            usize::try_from(self.mark_ms(RESTORED_MARK)?.max(0) / MILLISECONDS_PER_SECOND).ok()?;
        let (from_s, to_s) = (
            restore_s + RESTORED_SHARE_FROM_S,
            restore_s + RESTORED_SHARE_TO_S,
        );
        let mut on_restored = 0_u64;
        let mut total = 0_u64;
        for (link, stats) in &self.box_side.links {
            let packets: u64 = stats
                .data_packets_per_second
                .iter()
                .skip(from_s)
                .take(to_s - from_s)
                .sum();
            total += packets;
            if *link.ip() == RESTORED_LINK {
                on_restored += packets;
            }
        }
        (total > 0).then(|| to_f64(on_restored) * 100.0 / to_f64(total))
    }

    /// What the sender or the receiver (`role`) cost, as marked at the end of the run.
    #[must_use]
    pub fn resources(&self, role: &str) -> Option<ProcessResources> {
        self.marks
            .iter()
            .rev()
            .find_map(|mark| ProcessResources::parse(&mark.label, role))
    }

    /// Bytes a raw stream sent (`raw stream 1 started at <bit/s> bit/s for <s>s`), if any.
    #[must_use]
    pub fn raw_stream_bytes(&self) -> Option<u64> {
        let label = self
            .marks
            .iter()
            .find_map(|mark| mark.label.strip_prefix(RAW_STREAM_PREFIX))?;
        let (rate, rest) = label.split_once(" bit/s for ")?;
        let seconds: u64 = rest.strip_suffix('s')?.parse().ok()?;
        Some(rate.parse::<u64>().ok()? * seconds / BITS_PER_BYTE)
    }

    /// How long stream 1's encoder ran (`stream 1 started at <rate> for <s>s`), in seconds.
    #[must_use]
    pub fn stream_seconds(&self) -> Option<u64> {
        let label = self
            .marks
            .iter()
            .find_map(|mark| mark.label.strip_prefix(STREAM_PREFIX))?;
        let (_, seconds) = label.rsplit_once(" for ")?;
        seconds.strip_suffix('s')?.parse().ok()
    }

    /// Bytes the SRT listener delivered (MPEG-TS: video, damaged frames and audio included),
    /// averaged over the time stream 1 ran, in kbit/s.
    #[must_use]
    pub fn delivered_kbps(&self) -> Option<f64> {
        let seconds = self.stream_seconds().filter(|seconds| *seconds > 0)?;
        Some(to_f64(self.received_bytes * BITS_PER_BYTE) / to_f64(seconds) / BITS_PER_KILOBIT)
    }

    /// Video that arrived intact (frames with exactly the bytes the encoder made), averaged
    /// over the time stream 1 ran, in kbit/s. Unlike [`delivered_kbps`](Self::delivered_kbps),
    /// damaged frames and the audio do not count: this is what a viewer could watch.
    #[must_use]
    pub fn intact_kbps(&self) -> Option<f64> {
        let seconds = self.stream_seconds().filter(|seconds| *seconds > 0)?;
        let frames = self.frames.filter(|frames| frames.intact_bytes > 0)?;
        Some(to_f64(frames.intact_bytes * BITS_PER_BYTE) / to_f64(seconds) / BITS_PER_KILOBIT)
    }

    /// How long the recording took to be whole on the server after it was written (the
    /// stream's end), in seconds; `None` if it never was, or the run recorded nothing.
    #[must_use]
    pub fn recording_whole_after_s(&self) -> Option<f64> {
        let written_ms = self.mark_ms(RECORDING_WRITTEN_MARK)?;
        let delivered_ms = self.mark_ms(RECORDING_DELIVERED_MARK)?;
        let whole = self.recording.is_some_and(|recording| recording.whole());
        let after_ms = u64::try_from(delivered_ms - written_ms).unwrap_or(0);
        whole.then(|| to_f64(after_ms) / MILLISECONDS_PER_SECOND_F64)
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

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: u64) -> f64 {
    value as f64
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
