//! What goes back to the SRT caller: every SRT packet from the receiver, nothing of SRTLA's.

mod support;

use bonding_protocol::srtla::PacketType;
use support::{
    CALLER, FakeIo, Sent, TestResult, answer, connect, data, link, sender_on, srt_control,
    srtla_ack,
};

#[test]
fn srt_packets_go_back_to_the_last_caller_address() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    sender.handle_srt_datagram(0, CALLER, &data(1), &mut io);
    io.take_sent();
    let one = link(&sender, 1)?;
    for kind in [
        PacketType::SrtHandshake,
        PacketType::SrtKeepalive,
        PacketType::SrtShutdown,
        PacketType::SrtAck,
        PacketType::SrtNak,
    ] {
        sender.handle_link_datagram(0, one, &srt_control(kind), &mut io);
    }
    sender.handle_link_datagram(0, one, &data(9), &mut io);
    let sent = io.take_sent();
    assert_eq!(sent.len(), 6);
    assert!(
        sent.iter()
            .all(|sent| matches!(sent, Sent::ToSrt(to, _) if *to == CALLER))
    );
    Ok(())
}

#[test]
fn srtla_packets_stay_in_the_sender() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    sender.handle_srt_datagram(0, CALLER, &data(1), &mut io);
    io.take_sent();
    let one = link(&sender, 1)?;
    sender.handle_link_datagram(0, one, &answer(PacketType::Keepalive), &mut io);
    sender.handle_link_datagram(0, one, &srtla_ack(&[1]), &mut io);
    sender.handle_link_datagram(0, one, &answer(PacketType::Reg3), &mut io);
    sender.handle_link_datagram(0, one, &answer(PacketType::RegNgp), &mut io);
    assert!(io.to_srt().is_empty());
    Ok(())
}

#[test]
fn other_srtla_answers_reach_the_caller_as_upstream_does() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    sender.handle_srt_datagram(0, CALLER, &data(1), &mut io);
    sender.handle_link_datagram(0, link(&sender, 1)?, &answer(PacketType::RegErr), &mut io);
    assert_eq!(io.to_srt(), vec![answer(PacketType::RegErr)]);
    Ok(())
}

#[test]
fn nothing_goes_back_before_the_caller_spoke() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    sender.handle_link_datagram(
        0,
        link(&sender, 1)?,
        &srt_control(PacketType::SrtHandshake),
        &mut io,
    );
    assert!(io.to_srt().is_empty());
    Ok(())
}

#[test]
fn datagrams_are_forwarded_byte_for_byte() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let mut big = data(5);
    big.resize(1_316, 0x5C);
    sender.handle_srt_datagram(0, CALLER, &big, &mut io);
    assert_eq!(io.on_link(link(&sender, 1)?), vec![big]);
    Ok(())
}
