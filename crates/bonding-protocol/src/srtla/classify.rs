//! Recognising a packet, with the exact lengths registration packets must have.

use super::lengths::{REG1_LEN, REG2_LEN, REG3_LEN};
use super::packet_type::PacketType;
use super::read::raw_type;

/// Classifies a packet, enforcing the exact lengths of registration packets.
#[must_use]
pub fn classify(packet: &[u8]) -> Option<PacketType> {
    let kind = PacketType::from_raw(raw_type(packet)?)?;
    let length_ok = match kind {
        PacketType::Reg1 => packet.len() == REG1_LEN,
        PacketType::Reg2 => packet.len() == REG2_LEN,
        PacketType::Reg3 => packet.len() == REG3_LEN,
        _ => true,
    };
    length_ok.then_some(kind)
}
