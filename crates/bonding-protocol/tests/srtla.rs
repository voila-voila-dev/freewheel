//! SRTLA wire format: types, lengths, sequence numbers.

use bonding_protocol::srtla::*;

#[test]
fn reads_big_endian_type() {
    assert_eq!(raw_type(&[0x90, 0x00]), Some(0x9000));
    assert_eq!(classify(&[0x90, 0x00]), Some(PacketType::Keepalive));
}

#[test]
fn rejects_short_buffers() {
    assert_eq!(raw_type(&[0x90]), None);
    assert_eq!(data_sequence_number(&[0, 0, 1]), None);
}

#[test]
fn registration_requires_exact_length() {
    let mut reg1 = [0u8; REG1_LEN];
    reg1[..2].copy_from_slice(&0x9200u16.to_be_bytes());
    assert_eq!(classify(&reg1), Some(PacketType::Reg1));
    assert_eq!(classify(&reg1[..REG1_LEN - 1]), None);
    assert_eq!(classify(&[0x92, 0x02]), Some(PacketType::Reg3));
}

#[test]
fn data_packets_have_sequence_numbers_control_packets_do_not() {
    assert_eq!(
        data_sequence_number(&[0x00, 0x00, 0x01, 0x02]),
        Some(0x0102)
    );
    assert_eq!(data_sequence_number(&[0x80, 0x02, 0x00, 0x00]), None);
}
