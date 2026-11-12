//! What reaches SRT, and which link SRT's answers go back on.

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_receiver::srtla::Receiver;
use support::{FakeIo, Sent, TestResult, answer, data, link, reg1, register, srt_control};

#[test]
fn packets_from_a_link_reach_srt_unchanged_on_one_socket_per_group() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1), link(2)])?;
    let handshake = srt_control(PacketType::SrtHandshake);
    receiver.handle_link_datagram(1, link(1), &handshake, &mut io);
    receiver.handle_link_datagram(1, link(2), &data(5), &mut io);
    assert_eq!(io.to_srt(), [handshake, data(5)]);
    assert_eq!(io.opened, [group], "opened on the first SRT packet, once");
    Ok(())
}

#[test]
fn strangers_get_silence() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    for packet in [
        data(1),
        answer(PacketType::Keepalive),
        answer(PacketType::Reg3),
        srt_control(PacketType::SrtHandshake),
        vec![0x80],
        Vec::new(),
    ] {
        receiver.handle_link_datagram(1, link(5), &packet, &mut io);
    }
    assert_eq!(io.sent, []);
    Ok(())
}

#[test]
fn the_address_that_sent_reg1_is_not_a_link() {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    receiver.handle_link_datagram(0, link(1), &reg1(1), &mut io);
    io.take_sent();
    receiver.handle_link_datagram(0, link(1), &data(1), &mut io);
    receiver.handle_link_datagram(0, link(1), &answer(PacketType::Keepalive), &mut io);
    assert_eq!(io.sent, []);
}

#[test]
fn short_packets_are_not_forwarded() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    receiver.handle_link_datagram(1, link(1), &data(1)[..15], &mut io);
    receiver.handle_link_datagram(1, link(1), &answer(PacketType::Reg3), &mut io);
    assert_eq!(io.sent, []);
    Ok(())
}

#[test]
fn srtla_types_of_sixteen_bytes_or_more_are_forwarded_like_upstream() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    let ack = srt_control(PacketType::SrtlaAck);
    receiver.handle_link_datagram(1, link(1), &ack, &mut io);
    assert_eq!(io.to_srt(), [ack]);
    Ok(())
}

#[test]
fn srt_acks_go_back_on_every_link() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1), link(2), link(3)])?;
    receiver.handle_link_datagram(1, link(1), &data(1), &mut io);
    io.take_sent();
    let ack = srt_control(PacketType::SrtAck);
    receiver.handle_srt_datagram(group, &ack, &mut io);
    let expected: Vec<Sent> = [3, 2, 1]
        .map(|k| Sent::ToLink(link(k), ack.clone()))
        .to_vec();
    assert_eq!(io.sent, expected);
    Ok(())
}

#[test]
fn other_srt_packets_go_back_on_the_link_that_last_carried_one() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1), link(2)])?;
    receiver.handle_link_datagram(1, link(1), &data(1), &mut io);
    receiver.handle_link_datagram(1, link(2), &data(2), &mut io);
    receiver.handle_link_datagram(1, link(2), &answer(PacketType::Keepalive), &mut io);
    receiver.handle_link_datagram(1, link(1), &data(1)[..8], &mut io);
    io.take_sent();
    let nak = srt_control(PacketType::SrtNak);
    receiver.handle_srt_datagram(group, &nak, &mut io);
    assert_eq!(
        io.sent,
        [Sent::ToLink(link(2), nak)],
        "keepalives and short packets do not count"
    );
    Ok(())
}
