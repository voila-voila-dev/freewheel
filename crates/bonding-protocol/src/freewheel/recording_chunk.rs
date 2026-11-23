//! Bytes of the recording, and where they belong in it.

/// Bytes of the full-quality recording at `offset`, borrowed from the datagram they came in.
///
/// The offset makes a resend idempotent: the server writes the bytes where they belong,
/// whichever copy arrives, as many times as it arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordingChunk<'a> {
    /// Where the first byte goes in the recording, in bytes from its start.
    pub offset: u64,
    /// The bytes, from 1 to [`MAX_RECORDING_CHUNK_LEN`](super::MAX_RECORDING_CHUNK_LEN).
    pub bytes: &'a [u8],
}

impl RecordingChunk<'_> {
    /// The offset just past its last byte.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.offset.saturating_add(self.bytes.len() as u64)
    }
}
