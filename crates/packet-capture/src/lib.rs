//! Reading pcap captures of UDP traffic, to study upstream SRTLA on the wire.
//!
//! - [`PcapFile`]: frames of a classic pcap file (Ethernet or Linux cooked capture).
//! - [`UdpDatagram`]: an IPv4 UDP datagram decoded from one frame.
//! - [`SrtlaReport`]: what a capture says about registration, keepalives, acknowledgements
//!   and how traffic is spread across links.
//! - [`FrameCheck`]: the received video compared with what was sent, frame by frame.
//! - [`FrameClock`], [`FrameTime`], [`FrameLatency`]: when each frame left the encoder and
//!   reached the server, and the latency in between.
//! - [`LabRun`]: everything one lab scenario produced (two captures, marks, SRT statistics).
//!
//! Lab and scenarios: `labs/srtla-capture/`. Findings: `docs/technical/protocol/srtla.md`.

mod capture_error;
mod captured_frame;
mod frame_check;
mod frame_clock;
mod frame_latency;
mod frame_time;
mod lab_run;
mod lab_run_error;
mod link_layer;
mod link_stats;
mod mark;
mod pcap_file;
mod process_resources;
mod srt_statistics;
mod srtla_event;
mod srtla_report;
mod statistics;
mod udp_datagram;

pub use capture_error::CaptureError;
pub use captured_frame::CapturedFrame;
pub use frame_check::{FrameCheck, FrameDigest, FrameDigests};
pub use frame_clock::FrameClock;
pub use frame_latency::FrameLatency;
pub use frame_time::FrameTime;
pub use lab_run::{LAB_RECEIVER_PORT, LabRun};
pub use lab_run_error::LabRunError;
pub use link_layer::LinkLayer;
pub use link_stats::LinkStats;
pub use mark::Mark;
pub use pcap_file::PcapFile;
pub use process_resources::ProcessResources;
pub use srt_statistics::SrtStatistics;
pub use srtla_event::SrtlaEvent;
pub use srtla_report::SrtlaReport;
pub use statistics::{median, percentile_of_sorted, spread};
pub use udp_datagram::UdpDatagram;
