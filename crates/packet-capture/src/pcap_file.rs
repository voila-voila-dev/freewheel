//! Frames of a classic pcap file, read without copying.

use crate::{CaptureError, CapturedFrame, LinkLayer};

const MAGIC_MICROSECONDS_LITTLE_ENDIAN: [u8; 4] = [0xD4, 0xC3, 0xB2, 0xA1];
const GLOBAL_HEADER_BYTES: usize = 24;
const RECORD_HEADER_BYTES: usize = 16;
const LINK_TYPE_OFFSET: usize = 20;
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;

/// A classic pcap file (little-endian, microsecond timestamps; Ethernet or Linux cooked frames).
#[derive(Debug, Clone, Copy)]
pub struct PcapFile<'a> {
    bytes: &'a [u8],
    link_layer: LinkLayer,
}

impl<'a> PcapFile<'a> {
    /// Checks the global header.
    ///
    /// # Errors
    /// The format or the link type is not supported, or the header is cut short.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, CaptureError> {
        let header = bytes
            .get(..GLOBAL_HEADER_BYTES)
            .ok_or(CaptureError::Truncated { offset: 0 })?;
        if header[..4] != MAGIC_MICROSECONDS_LITTLE_ENDIAN {
            return Err(CaptureError::UnsupportedFormat);
        }
        let link_type = read_u32(header, LINK_TYPE_OFFSET);
        let link_layer = LinkLayer::from_link_type(link_type)
            .ok_or(CaptureError::UnsupportedLinkType { link_type })?;
        Ok(Self { bytes, link_layer })
    }

    /// Every frame, in capture order.
    ///
    /// # Errors
    /// The file ends in the middle of a frame.
    pub fn frames(&self) -> Result<Vec<CapturedFrame<'a>>, CaptureError> {
        let mut frames = Vec::new();
        let mut offset = GLOBAL_HEADER_BYTES;
        while offset < self.bytes.len() {
            let record = self
                .bytes
                .get(offset..offset + RECORD_HEADER_BYTES)
                .ok_or(CaptureError::Truncated { offset })?;
            let seconds = u64::from(read_u32(record, 0));
            let microseconds = u64::from(read_u32(record, 4));
            let captured_bytes = read_u32(record, 8) as usize;
            let start = offset + RECORD_HEADER_BYTES;
            let bytes = self
                .bytes
                .get(start..start + captured_bytes)
                .ok_or(CaptureError::Truncated { offset: start })?;
            frames.push(CapturedFrame {
                at_us: seconds * MICROSECONDS_PER_SECOND + microseconds,
                link_layer: self.link_layer,
                bytes,
            });
            offset = start + captured_bytes;
        }
        Ok(frames)
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut word = [0u8; 4];
    word.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(word)
}
