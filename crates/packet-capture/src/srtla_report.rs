//! What a capture says about upstream SRTLA: registration, keepalives, ACKs, link usage.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::net::SocketAddrV4;

use bonding_protocol::srtla::{ID_LEN, PacketType, classify, data_sequence_number, raw_type};

use crate::{LinkStats, SrtlaEvent, UdpDatagram};

const MICROSECONDS_PER_MILLISECOND: u64 = 1_000;
const MILLISECONDS_PER_SECOND: u64 = 1_000;
const SRTLA_ACK_HEADER_BYTES: usize = 4;
const SEQUENCE_NUMBER_BYTES: usize = 4;
const HALF_ID_LEN: usize = ID_LEN / 2;

/// What a capture says about upstream SRTLA.
#[derive(Debug, Clone, Default)]
pub struct SrtlaReport {
    /// UDP port the receiver listens on; tells uplink from downlink.
    pub receiver_port: u16,
    /// Time of the first packet, in microseconds since the Unix epoch.
    pub start_us: u64,
    /// Distinct group ids handed out by the receiver (in its REG2 answers).
    pub group_ids: BTreeSet<Vec<u8>>,
    /// Per sender-side link.
    pub links: BTreeMap<SocketAddrV4, LinkStats>,
    /// Registration, keepalive and error packets, in order.
    pub timeline: Vec<SrtlaEvent>,
    /// Control packets of types SRTLA does not name (other SRT control), by raw type.
    pub other_control: BTreeMap<u16, u64>,
    /// For each REG2 answered by the receiver: did its first half equal the sender's REG1 id?
    pub reg2_keeps_sender_half: Vec<bool>,
    /// Last REG1 id sent by the box.
    last_reg1_id: Option<Vec<u8>>,
    /// Time of the last uplink packet per link, in milliseconds.
    last_uplink_ms: BTreeMap<SocketAddrV4, u64>,
    /// Time of the last keepalive per link, in milliseconds.
    last_keepalive_ms: BTreeMap<SocketAddrV4, u64>,
    /// Sequence numbers sent per link, for ACK matching.
    sent_on: BTreeMap<SocketAddrV4, BTreeSet<u32>>,
}

impl SrtlaReport {
    /// Builds the report from datagrams in capture order.
    #[must_use]
    pub fn build(receiver_port: u16, datagrams: &[UdpDatagram<'_>]) -> Self {
        let start_us = datagrams.first().map_or(0, |d| d.at_us);
        let mut report = Self {
            receiver_port,
            start_us,
            ..Self::default()
        };
        for datagram in datagrams {
            let at_ms = (datagram.at_us - start_us) / MICROSECONDS_PER_MILLISECOND;
            if datagram.destination.port() == receiver_port {
                report.record_uplink(datagram.source, at_ms, datagram.payload, datagram.length);
            } else if datagram.source.port() == receiver_port {
                report.record_downlink(datagram.destination, at_ms, datagram.payload);
            }
        }
        for (link, stats) in &mut report.links {
            if let Some(sent) = report.sent_on.get(link) {
                stats.data_sequence_numbers = sent.iter().copied().collect();
            }
        }
        report
    }

    fn record_uplink(&mut self, link: SocketAddrV4, at_ms: u64, payload: &[u8], length: usize) {
        let previous_ms = self.last_uplink_ms.insert(link, at_ms);
        let stats = self.links.entry(link).or_default();
        if let Some(previous_ms) = previous_ms {
            stats.longest_uplink_silence_ms = stats
                .longest_uplink_silence_ms
                .max(at_ms.saturating_sub(previous_ms));
        }
        stats.uplink_span_ms = Some(
            stats
                .uplink_span_ms
                .map_or((at_ms, at_ms), |(first, _)| (first, at_ms)),
        );

        if let Some(sequence_number) = data_sequence_number(payload) {
            stats.data_packets += 1;
            stats.data_bytes += length as u64;
            let second = usize::try_from(at_ms / MILLISECONDS_PER_SECOND).unwrap_or(usize::MAX);
            if stats.data_packets_per_second.len() <= second {
                stats.data_packets_per_second.resize(second + 1, 0);
            }
            stats.data_packets_per_second[second] += 1;
            self.sent_on
                .entry(link)
                .or_default()
                .insert(sequence_number);
            return;
        }
        let Some(kind) = classify(payload) else {
            self.count_other_control(payload);
            return;
        };
        *stats.uplink_control.entry(kind as u16).or_default() += 1;
        match kind {
            PacketType::Keepalive => {
                if let Some(previous_ms) = self.last_keepalive_ms.insert(link, at_ms) {
                    stats
                        .keepalive_gaps_ms
                        .push(at_ms.saturating_sub(previous_ms));
                }
            }
            PacketType::Reg1 => self.last_reg1_id = Some(payload[2..].to_vec()),
            _ => {}
        }
        if is_timeline_worthy(kind) {
            self.timeline.push(SrtlaEvent {
                at_ms,
                link,
                uplink: true,
                kind,
                bytes: payload.len(),
            });
        }
    }

    fn record_downlink(&mut self, link: SocketAddrV4, at_ms: u64, payload: &[u8]) {
        let Some(kind) = classify(payload) else {
            self.count_other_control(payload);
            return;
        };
        let stats = self.links.entry(link).or_default();
        *stats.downlink_control.entry(kind as u16).or_default() += 1;
        match kind {
            PacketType::SrtlaAck => {
                let acked: Vec<u32> = payload[SRTLA_ACK_HEADER_BYTES..]
                    .chunks_exact(SEQUENCE_NUMBER_BYTES)
                    .map(|word| u32::from_be_bytes([word[0], word[1], word[2], word[3]]))
                    .collect();
                *stats.srtla_ack_sizes.entry(acked.len()).or_default() += 1;
                let sent_here = self.sent_on.get(&link);
                for sequence_number in acked {
                    stats.srtla_acked += 1;
                    if sent_here.is_some_and(|sent| sent.contains(&sequence_number)) {
                        stats.srtla_acked_on_same_link += 1;
                    }
                }
            }
            PacketType::Reg2 => {
                self.group_ids.insert(payload[2..].to_vec());
                if let Some(sender_id) = &self.last_reg1_id {
                    self.reg2_keeps_sender_half
                        .push(payload[2..2 + HALF_ID_LEN] == sender_id[..HALF_ID_LEN]);
                }
            }
            _ => {}
        }
        if is_timeline_worthy(kind) {
            self.timeline.push(SrtlaEvent {
                at_ms,
                link,
                uplink: false,
                kind,
                bytes: payload.len(),
            });
        }
    }

    fn count_other_control(&mut self, payload: &[u8]) {
        let raw = raw_type(payload).unwrap_or_default();
        *self.other_control.entry(raw).or_default() += 1;
    }
}

fn is_timeline_worthy(kind: PacketType) -> bool {
    !matches!(
        kind,
        PacketType::SrtAck | PacketType::SrtlaAck | PacketType::SrtNak
    )
}

impl fmt::Display for SrtlaReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "## Links (receiver port {})", self.receiver_port)?;
        let total_data: u64 = self.links.values().map(|s| s.data_packets).sum();
        for (link, stats) in &self.links {
            let share_per_mille = (stats.data_packets * 1_000)
                .checked_div(total_data)
                .unwrap_or(0);
            writeln!(f, "\n### {link}")?;
            writeln!(
                f,
                "- data: {} packets ({}.{} %), {} bytes",
                stats.data_packets,
                share_per_mille / 10,
                share_per_mille % 10,
                stats.data_bytes
            )?;
            writeln!(
                f,
                "- data packets per second: {:?}",
                stats.data_packets_per_second
            )?;
            writeln!(
                f,
                "- uplink control: {}",
                named_counts(&stats.uplink_control)
            )?;
            writeln!(
                f,
                "- downlink control: {}",
                named_counts(&stats.downlink_control)
            )?;
            writeln!(
                f,
                "- SRTLA ACK sizes (sequence numbers → ACKs): {:?}",
                stats.srtla_ack_sizes
            )?;
            writeln!(
                f,
                "- SRTLA-acked: {}, of which sent on this link: {}",
                stats.srtla_acked, stats.srtla_acked_on_same_link
            )?;
            writeln!(
                f,
                "- keepalive gaps (ms): {}",
                summary(&stats.keepalive_gaps_ms)
            )?;
            writeln!(
                f,
                "- longest uplink silence: {} ms, uplink span: {:?}",
                stats.longest_uplink_silence_ms, stats.uplink_span_ms
            )?;
        }
        writeln!(f, "\n## Groups handed out: {}", self.group_ids.len())?;
        writeln!(
            f,
            "## REG2 keeps the sender's half of the id: {:?}",
            self.reg2_keeps_sender_half
        )?;
        writeln!(
            f,
            "## Other SRT control (raw type → packets): {}",
            named_counts(&self.other_control)
        )?;
        writeln!(
            f,
            "\n## Timeline (registration, keepalives, handshakes, errors)"
        )?;
        for event in &self.timeline {
            let arrow = if event.uplink {
                "box → server"
            } else {
                "server → box"
            };
            writeln!(
                f,
                "- {:>6} ms  {}  {arrow}  {:?} ({} B)",
                event.at_ms, event.link, event.kind, event.bytes
            )?;
        }
        Ok(())
    }
}

fn named_counts(counts: &BTreeMap<u16, u64>) -> String {
    counts
        .iter()
        .map(|(raw, count)| match PacketType::from_raw(*raw) {
            Some(kind) => format!("{kind:?}×{count}"),
            None => format!("{raw:#06x}×{count}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn summary(values: &[u64]) -> String {
    let (Some(min), Some(max)) = (values.iter().min(), values.iter().max()) else {
        return "none".to_owned();
    };
    let mean = values.iter().sum::<u64>() / values.len() as u64;
    format!("{} gaps, min {min}, mean {mean}, max {max}", values.len())
}
