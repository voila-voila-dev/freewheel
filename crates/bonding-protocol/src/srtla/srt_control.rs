//! Any other SRT control packet: forwarded unchanged, never read.

/// An SRT control packet (top bit set, at least 16 bytes) that SRTLA does not interpret:
/// handshake, keepalive, shutdown, ACKACK, unknown types, and ACKs or NAKs too malformed to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtControl<'a> {
    bytes: &'a [u8],
}

impl<'a> SrtControl<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// Its raw 16-bit type.
    #[must_use]
    pub fn raw_type(&self) -> u16 {
        super::read::raw_type(self.bytes).unwrap_or_default()
    }

    /// The whole datagram, to forward as is.
    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}
