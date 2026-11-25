//! Why each lost video frame was lost: not enough capacity, isolated packet losses, or
//! something the transport did (a link that died under it, a full queue, a late packet).
//!
//! It tells which mechanism can win frames back: forward error correction repairs isolated
//! losses; only a lower encoder bitrate saves the seconds the links cannot carry.

use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::path::Path;

use bonding_protocol::freewheel;
use bonding_protocol::srtla::data_sequence_number;

use crate::{
    EmulatedUplinks, FrameDigests, FrameTime, LAB_RECEIVER_PORT, LabRunError, Mark, PcapFile,
    UdpDatagram,
};

const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const MICROSECONDS_PER_MILLISECOND: u64 = 1_000;
const BITS_PER_BYTE: f64 = 8.0;
/// Bytes on the uplink for one byte of compressed video, first transmissions only: MPEG-TS
/// with the 128 kbit/s audio (1.10 on the video), packets cut short at the end of each frame (a fifth are 392 or 580
/// bytes), then SRT (16), our data header (20), UDP, IPv4 and Ethernet (42) per packet, as
/// `netem` counts them. Measured on `trace-city` (CI run 44262198730, 3 runs): 1.18 for our
/// transport, 1.16 for SRTLA (no data header). One value for every pair keeps the demand the
/// same for all; the classification barely moves between 1.0 and 1.24.
pub const WIRE_BYTES_PER_VIDEO_BYTE: f64 = 1.18;
/// A frame's packets leave the box within this window around the time its last byte left the
/// encoder (a large keyframe takes ~30 packets, the caller sends them in a burst).
const FRAME_PACKETS_BEFORE_US: u64 = 50 * MICROSECONDS_PER_MILLISECOND;
const FRAME_PACKETS_AFTER_US: u64 = 10 * MICROSECONDS_PER_MILLISECOND;
/// How long after a short second the stream may still be catching up (`trace-drive`: every
/// frame lost 1–4 s after the capacity came back at twice the stream's rate).
pub const RECOVERY_SECONDS: usize = 5;

/// Why one frame was lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossCause {
    /// The links together carried less than the stream that second, the one before (its
    /// backlog) or the one after (the frame was still queued when the capacity went).
    Capacity,
    /// Capacity was enough again, at most [`RECOVERY_SECONDS`] after a second that was short,
    /// and the frame was not lost on the way: the stream catching up with what queued during
    /// the hole (SRT drops packets that are too late).
    Recovery,
    /// Capacity was enough, and some of its packets were lost on a link with random or bursty
    /// loss: what forward error correction would repair.
    IsolatedLoss,
    /// Capacity was enough, and some of its packets went into a link that was cut or dead.
    LinkDown,
    /// Capacity was enough, and some of its packets were lost on a link that drops nothing at
    /// random: its queue overflowed.
    Queue,
    /// Capacity was enough and every packet the box sent around it arrived: dropped at the box
    /// as late, or late for SRT.
    Late,
}

/// How many lost frames each cause explains, in one run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LossCauses {
    /// [`LossCause::Capacity`].
    pub capacity: usize,
    /// [`LossCause::Recovery`].
    pub recovery: usize,
    /// [`LossCause::IsolatedLoss`].
    pub isolated_loss: usize,
    /// [`LossCause::LinkDown`].
    pub link_down: usize,
    /// [`LossCause::Queue`].
    pub queue: usize,
    /// [`LossCause::Late`].
    pub late: usize,
    /// Seconds of the stream where the links together carried less than it.
    pub short_seconds: usize,
}

/// A data packet the box sent: when, on which link (`k` of `10.<k>.0.2`), and its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentPacket {
    /// Capture time on the box, in microseconds since the Unix epoch.
    pub at_us: u64,
    /// Link number.
    pub link: u8,
    /// Our transport's sequence (one per transmission), or SRT's for SRTLA (a retransmission
    /// reuses it: such a packet is lost only when no copy arrived).
    pub sequence: u32,
}

/// Everything [`LossCauses::classify`] needs from one run.
#[derive(Debug, Clone, Copy)]
pub struct LossEvidence<'a> {
    /// What the emulated network let each uplink carry.
    pub uplinks: &'a EmulatedUplinks,
    /// What the encoder sent.
    pub sent: &'a FrameDigests,
    /// What the SRT listener delivered.
    pub received: &'a FrameDigests,
    /// When each sent frame left the encoder.
    pub sent_times: &'a [FrameTime],
    /// Data packets the box sent, in capture order.
    pub box_packets: &'a [SentPacket],
    /// Sequences of the data packets that reached the server.
    pub delivered: &'a HashSet<u32>,
}

impl LossCauses {
    /// Loads `<directory>/<scenario>.{marks,sent.1.frames,received.frames,sent.1.times,
    /// box.pcap,server.pcap}` and classifies its lost frames. `None` when the run has no frame
    /// digests or times, or no `netem` marks (runs from before the lab wrote them).
    ///
    /// # Errors
    /// A capture or the marks file is missing or unreadable.
    pub fn load(directory: &Path, scenario: &str) -> Result<Option<Self>, LabRunError> {
        let file = |suffix: &str| directory.join(format!("{scenario}.{suffix}"));
        let uplinks = EmulatedUplinks::from_marks(&Mark::parse_all(&read_text(&file("marks"))?));
        if uplinks.is_empty() {
            return Ok(None);
        }
        let (Ok(sent), Ok(received), Ok(times)) = (
            read_text(&file("sent.1.frames")),
            read_text(&file("received.frames")),
            read_text(&file("sent.1.times")),
        ) else {
            return Ok(None);
        };
        let box_packets = uplink_data(&file("box.pcap"))?;
        let delivered = uplink_data(&file("server.pcap"))?
            .into_iter()
            .map(|packet| packet.sequence)
            .collect();
        Ok(Some(Self::classify(&LossEvidence {
            uplinks: &uplinks,
            sent: &FrameDigests::parse(&sent),
            received: &FrameDigests::parse(&received),
            sent_times: &FrameTime::parse_all(&times),
            box_packets: &box_packets,
            delivered: &delivered,
        })))
    }

    /// Classifies every lost frame ([`FrameDigests::damaged_in`]) of one run.
    #[must_use]
    pub fn classify(evidence: &LossEvidence<'_>) -> Self {
        let mut causes = Self::default();
        let time_of: std::collections::HashMap<i64, u64> = evidence
            .sent_times
            .iter()
            .map(|time| (time.pts, time.at_us))
            .collect();
        let Some(start_us) = evidence.sent_times.iter().map(|time| time.at_us).min() else {
            return causes;
        };
        let short = short_seconds(evidence, &time_of, start_us);
        causes.short_seconds = short.iter().filter(|short| **short).count();
        let is_short = |second: Option<usize>| {
            second.is_some_and(|second| short.get(second).copied().unwrap_or(false))
        };
        for frame in evidence.sent.damaged_in(evidence.received) {
            let Some(at_us) = time_of.get(&frame.pts).copied() else {
                continue;
            };
            let second = to_index((at_us.saturating_sub(start_us)) / MICROSECONDS_PER_SECOND);
            let cause = if is_short(second.checked_sub(1))
                || is_short(Some(second))
                || is_short(Some(second + 1))
            {
                LossCause::Capacity
            } else {
                match packet_cause(evidence, at_us) {
                    LossCause::Late
                        if (2..=RECOVERY_SECONDS)
                            .any(|before| is_short(second.checked_sub(before))) =>
                    {
                        LossCause::Recovery
                    }
                    cause => cause,
                }
            };
            causes.count(cause);
        }
        causes
    }

    /// Frames lost, all causes.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.capacity + self.recovery + self.isolated_loss + self.link_down + self.queue + self.late
    }

    const fn count(&mut self, cause: LossCause) {
        match cause {
            LossCause::Capacity => self.capacity += 1,
            LossCause::Recovery => self.recovery += 1,
            LossCause::IsolatedLoss => self.isolated_loss += 1,
            LossCause::LinkDown => self.link_down += 1,
            LossCause::Queue => self.queue += 1,
            LossCause::Late => self.late += 1,
        }
    }
}

/// For each second of the stream: did the links together carry less than it?
fn short_seconds(
    evidence: &LossEvidence<'_>,
    time_of: &std::collections::HashMap<i64, u64>,
    start_us: u64,
) -> Vec<bool> {
    let mut demand_bytes: Vec<u64> = Vec::new();
    for frame in &evidence.sent.frames {
        let Some(at_us) = time_of.get(&frame.pts) else {
            continue;
        };
        let second = to_index((at_us - start_us) / MICROSECONDS_PER_SECOND);
        if demand_bytes.len() <= second {
            demand_bytes.resize(second + 1, 0);
        }
        demand_bytes[second] += frame.size_bytes;
    }
    demand_bytes
        .iter()
        .enumerate()
        .map(|(second, bytes)| {
            let from_us = start_us + to_u64(second) * MICROSECONDS_PER_SECOND;
            let capacity_bps = evidence
                .uplinks
                .capacity_bps_between(from_us, from_us + MICROSECONDS_PER_SECOND);
            capacity_bps < to_f64(*bytes) * BITS_PER_BYTE * WIRE_BYTES_PER_VIDEO_BYTE
        })
        .collect()
}

/// When capacity was enough: what happened to the packets the box sent around the frame.
fn packet_cause(evidence: &LossEvidence<'_>, frame_at_us: u64) -> LossCause {
    let from_us = frame_at_us.saturating_sub(FRAME_PACKETS_BEFORE_US);
    let to_us = frame_at_us + FRAME_PACKETS_AFTER_US;
    let first = evidence
        .box_packets
        .partition_point(|packet| packet.at_us < from_us);
    let (mut dead, mut lossy, mut limited) = (false, false, false);
    for packet in evidence.box_packets[first..]
        .iter()
        .take_while(|packet| packet.at_us <= to_us)
        .filter(|packet| !evidence.delivered.contains(&packet.sequence))
    {
        let setting = evidence.uplinks.setting_at(packet.link, packet.at_us);
        dead |= setting.dead;
        lossy |= setting.lossy;
        limited |= !setting.dead && !setting.lossy;
    }
    if dead {
        LossCause::LinkDown
    } else if lossy {
        LossCause::IsolatedLoss
    } else if limited {
        LossCause::Queue
    } else {
        LossCause::Late
    }
}

/// Data packets toward the receiver in a capture: our transport's, or SRTLA's (plain SRT).
fn uplink_data(path: &Path) -> Result<Vec<SentPacket>, LabRunError> {
    let bytes = std::fs::read(path).map_err(|error| LabRunError::Unreadable {
        path: path.display().to_string(),
        error,
    })?;
    let frames = PcapFile::parse(&bytes)
        .and_then(|file| file.frames())
        .map_err(|error| LabRunError::Capture {
            path: path.display().to_string(),
            error,
        })?;
    Ok(frames
        .into_iter()
        .filter_map(UdpDatagram::decode)
        .filter(|datagram| datagram.destination.port() == LAB_RECEIVER_PORT)
        .filter_map(|datagram| {
            let sequence = match freewheel::Packet::decode(datagram.payload) {
                Ok(freewheel::Packet::Data(data)) => data.sequence,
                // Sealed (v1), the sequence is encrypted: no link to tie a frame's loss to.
                _ if freewheel::is_freewheel(datagram.payload)
                    || freewheel::is_sealed(datagram.payload) =>
                {
                    return None;
                }
                _ => data_sequence_number(datagram.payload)?,
            };
            Some(SentPacket {
                at_us: datagram.at_us,
                link: link_number(*datagram.source.ip()),
                sequence,
            })
        })
        .collect())
}

/// `10.<k>.0.2` → `k`: the lab numbers its modems by the second byte of their address.
const fn link_number(address: Ipv4Addr) -> u8 {
    address.octets()[1]
}

fn read_text(path: &Path) -> Result<String, LabRunError> {
    std::fs::read_to_string(path).map_err(|error| LabRunError::Unreadable {
        path: path.display().to_string(),
        error,
    })
}

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: u64) -> f64 {
    value as f64
}

const fn to_u64(value: usize) -> u64 {
    value as u64
}

fn to_index(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
