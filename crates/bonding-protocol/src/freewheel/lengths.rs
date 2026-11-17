//! Fixed lengths and markers of freewheel packets.

/// First byte of every freewheel v0 packet: `0xF` for freewheel, `0` for the version.
///
/// It keeps our packets apart from SRT and SRTLA on the same port: SRT data starts with a
/// top bit of 0, SRT control types are `0x80xx`–`0x8Fxx` in practice and SRTLA's `0x90xx`–`0x92xx`.
pub const MAGIC: u8 = 0xF0;
/// Length of the header every packet starts with: magic, kind, link, class, session.
pub const HEADER_LEN: usize = 12;
/// Length of a data packet before its payload: header, sequence, send time.
pub const DATA_HEADER_LEN: usize = HEADER_LEN + 8;
/// Length of an ACK before its ranges: header, then four 32-bit timing fields.
pub const ACK_HEADER_LEN: usize = HEADER_LEN + 16;
/// Length of one acknowledged range: first and last sequence, 32 bits each.
pub const SEQUENCE_RANGE_LEN: usize = 8;
/// Length of a keepalive: header and send time.
pub const KEEPALIVE_LEN: usize = HEADER_LEN + 4;
/// Length of a join request: the header alone.
pub const JOIN_LEN: usize = HEADER_LEN;
/// Length of a join acceptance: the header alone.
pub const ACCEPT_LEN: usize = HEADER_LEN;
/// Length of a close: header and reason.
pub const CLOSE_LEN: usize = HEADER_LEN + 1;
/// Largest datagram either side reads: one Ethernet MTU, as in SRTLA mode.
pub const MAX_DATAGRAM_LEN: usize = 1_500;
/// Largest payload one data packet carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_DATA_PAYLOAD_LEN: usize = MAX_DATAGRAM_LEN - DATA_HEADER_LEN;
/// Most ranges one ACK carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_ACK_RANGES: usize = (MAX_DATAGRAM_LEN - ACK_HEADER_LEN) / SEQUENCE_RANGE_LEN;
