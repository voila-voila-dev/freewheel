//! One frame as recorded in a pcap file.

use crate::LinkLayer;

/// One frame as recorded in a pcap file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapturedFrame<'a> {
    /// Capture time, in microseconds since the Unix epoch.
    pub at_us: u64,
    /// The header the frame starts with.
    pub link_layer: LinkLayer,
    /// Bytes of the frame, starting at the link-layer header.
    pub bytes: &'a [u8],
}
