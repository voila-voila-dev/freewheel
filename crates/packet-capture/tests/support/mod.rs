//! Shared helpers: load a capture and build its report.

use packet_capture::{PcapFile, SrtlaReport, UdpDatagram};

/// Port upstream `srtla_rec` listens on in the lab.
pub const RECEIVER_PORT: u16 = 5000;

/// Builds the report of a pcap file's bytes.
pub fn report_of(bytes: &[u8]) -> SrtlaReport {
    let Ok(file) = PcapFile::parse(bytes) else {
        unreachable!("fixture is a valid pcap file")
    };
    let Ok(frames) = file.frames() else {
        unreachable!("fixture is complete")
    };
    let datagrams: Vec<UdpDatagram<'_>> =
        frames.into_iter().filter_map(UdpDatagram::decode).collect();
    SrtlaReport::build(RECEIVER_PORT, &datagrams)
}
