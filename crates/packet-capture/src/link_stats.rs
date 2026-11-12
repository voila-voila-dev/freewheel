//! What one sender-side link carried during a capture.

use std::collections::BTreeMap;

/// What one sender-side link carried during a capture.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// SRT data packets sent by the box on this link.
    pub data_packets: u64,
    /// Data packets sent by the box on this link, per second since the start of the capture.
    pub data_packets_per_second: Vec<u64>,
    /// Payload bytes of those data packets.
    pub data_bytes: u64,
    /// Sequence numbers sent on this link, to check SRTLA ACKs against.
    pub data_sequence_numbers: Vec<u32>,
    /// Control packets sent by the box, by raw type.
    pub uplink_control: BTreeMap<u16, u64>,
    /// Control packets received by the box, by raw type.
    pub downlink_control: BTreeMap<u16, u64>,
    /// How many sequence numbers each SRTLA ACK carried → how many ACKs.
    pub srtla_ack_sizes: BTreeMap<usize, u64>,
    /// Sequence numbers acknowledged by SRTLA ACKs on this link.
    pub srtla_acked: u64,
    /// Of those, how many had been sent on this same link.
    pub srtla_acked_on_same_link: u64,
    /// Gaps between consecutive keepalives sent by the box, in milliseconds.
    pub keepalive_gaps_ms: Vec<u64>,
    /// Longest time without any uplink packet, in milliseconds.
    pub longest_uplink_silence_ms: u64,
    /// First and last uplink packet, in milliseconds since the start of the capture.
    pub uplink_span_ms: Option<(u64, u64)>,
}
