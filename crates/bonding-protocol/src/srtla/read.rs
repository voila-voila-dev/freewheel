//! Reading fields from raw packets, without copying.

use super::lengths::SEQUENCE_NUMBER_LEN;

/// Reads the raw 16-bit packet type.
#[must_use]
pub fn raw_type(packet: &[u8]) -> Option<u16> {
    match packet {
        [hi, lo, ..] => Some(u16::from_be_bytes([*hi, *lo])),
        _ => None,
    }
}

/// Reads the SRT sequence number of a data packet.
/// Returns `None` for control packets (top bit set) or short buffers.
#[must_use]
pub fn data_sequence_number(packet: &[u8]) -> Option<u32> {
    word_at(packet, 0).filter(|word| word & (1 << 31) == 0)
}

/// Reads the big-endian 32-bit word starting at `offset`.
pub(crate) fn word_at(packet: &[u8], offset: usize) -> Option<u32> {
    let bytes = packet.get(offset..offset.checked_add(SEQUENCE_NUMBER_LEN)?)?;
    let word: [u8; SEQUENCE_NUMBER_LEN] = bytes.try_into().ok()?;
    Some(u32::from_be_bytes(word))
}
