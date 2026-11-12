//! Reading pcap captures of UDP traffic, to study upstream SRTLA on the wire.
//!
//! - [`PcapFile`]: frames of a classic pcap file (Ethernet or Linux cooked capture).
//! - [`UdpDatagram`]: an IPv4 UDP datagram decoded from one frame.
//! - [`SrtlaReport`]: what a capture says about registration, keepalives, acknowledgements
//!   and how traffic is spread across links.
//! - [`FrameCheck`]: the received video compared with what was sent, frame by frame.
//! - [`LabRun`]: everything one lab scenario produced (two captures, marks, SRT statistics).
//!
//! Lab and scenarios: `labs/srtla-capture/`. Findings: `docs/technical/protocol/srtla.md`.

mod capture_error;
mod captured_frame;
mod frame_check;
mod lab_run;
mod lab_run_error;
mod link_layer;
mod link_stats;
mod mark;
mod pcap_file;
mod srt_statistics;
mod srtla_event;
mod srtla_report;
mod udp_datagram;

pub use capture_error::CaptureError;
pub use captured_frame::CapturedFrame;
pub use frame_check::{FrameCheck, FrameDigest, FrameDigests};
pub use lab_run::{LAB_RECEIVER_PORT, LabRun};
pub use lab_run_error::LabRunError;
pub use link_layer::LinkLayer;
pub use link_stats::LinkStats;
pub use mark::Mark;
pub use pcap_file::PcapFile;
pub use srt_statistics::SrtStatistics;
pub use srtla_event::SrtlaEvent;
pub use srtla_report::SrtlaReport;
pub use udp_datagram::UdpDatagram;
