//! Reading pcap files: format checks and truncation.

use packet_capture::{CaptureError, PcapFile};

const FIXTURE: &[u8] = include_bytes!("fixtures/steady-start.pcap");

#[test]
fn reads_every_frame_of_the_fixture() {
    let frames = PcapFile::parse(FIXTURE).and_then(|file| file.frames());
    assert_eq!(frames.map(|frames| frames.len()), Ok(120));
}

#[test]
fn rejects_other_formats() {
    let mut bytes = FIXTURE.to_vec();
    bytes[0] = 0;
    assert!(matches!(
        PcapFile::parse(&bytes),
        Err(CaptureError::UnsupportedFormat)
    ));
    assert!(matches!(
        PcapFile::parse(&bytes[..10]),
        Err(CaptureError::Truncated { offset: 0 })
    ));
}

#[test]
fn reports_a_frame_cut_short() {
    let cut = &FIXTURE[..FIXTURE.len() - 1];
    let frames = PcapFile::parse(cut).and_then(|file| file.frames());
    assert!(matches!(frames, Err(CaptureError::Truncated { .. })));
}
