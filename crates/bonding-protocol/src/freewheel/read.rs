//! Reading big-endian fields from raw packets, without copying.

/// The big-endian 32-bit word starting at `offset`.
pub(crate) fn u32_at(packet: &[u8], offset: usize) -> Option<u32> {
    let bytes = packet.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

/// The big-endian 64-bit word starting at `offset`.
pub(crate) fn u64_at(packet: &[u8], offset: usize) -> Option<u64> {
    let bytes = packet.get(offset..offset.checked_add(8)?)?;
    Some(u64::from_be_bytes(bytes.try_into().ok()?))
}
