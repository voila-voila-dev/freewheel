//! SRTLA ACKs: every 10th data packet of a link, the 10 numbers, on that link.

mod support;

use bonding_protocol::srtla::{Packet, PacketType, SequenceNumber};
use bonding_receiver::srtla::Receiver;
use support::{FakeIo, TestResult, data, link, register, srt_control};

fn srtla_acks(io: &FakeIo, k: u8) -> Vec<Vec<u32>> {
    io.to_link(link(k))
        .iter()
        .filter_map(|bytes| match Packet::decode(bytes) {
            Ok(Packet::SrtlaAck(ack)) => {
                Some(ack.sequence_numbers().map(SequenceNumber::get).collect())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_tenth_data_packet_acknowledges_the_last_ten_of_that_link() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    register(&mut receiver, &mut io, 0, 1, &[link(1), link(2)])?;
    for number in 100..125 {
        let k = if number % 3 == 0 { 2 } else { 1 };
        receiver.handle_link_datagram(1, link(k), &data(number), &mut io);
        receiver.handle_link_datagram(1, link(k), &srt_control(PacketType::SrtAckAck), &mut io);
    }
    let on_link_1: Vec<u32> = (100..125).filter(|n| n % 3 != 0).collect();
    assert_eq!(srtla_acks(&io, 1), [on_link_1[..10].to_vec()]);
    assert_eq!(
        srtla_acks(&io, 2),
        Vec::<Vec<u32>>::new(),
        "8 packets: no ACK yet"
    );
    let ack_len = io.to_link(link(1)).first().map(Vec::len);
    assert_eq!(ack_len, Some(44));
    Ok(())
}

#[test]
fn a_partial_batch_is_never_flushed() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    for number in 0..19 {
        receiver.handle_link_datagram(1, link(1), &data(number), &mut io);
    }
    receiver.housekeeping(5, &mut io);
    receiver.housekeeping(9, &mut io);
    assert_eq!(srtla_acks(&io, 1), [(0..10).collect::<Vec<u32>>()]);
    Ok(())
}
