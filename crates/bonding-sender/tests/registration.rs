//! Registration: REG2 everywhere, `REG_NGP`, REG1 on one link, REG2 back, REG2 everywhere, REG3s.

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_sender::srtla::SenderEvent;
use support::{
    FakeIo, RECEIVER, Sent, TestResult, answer, connect, full_id, link, proposed_id, registration,
    sender_on, source,
};

#[test]
fn at_start_every_link_sends_reg2_with_the_random_id() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    sender.housekeeping(0, &mut io);
    let reg2 = registration(PacketType::Reg2, &proposed_id());
    for k in [1, 2] {
        assert_eq!(io.on_link(link(&sender, k)?), vec![reg2.clone()]);
    }
    assert!(
        io.sent
            .iter()
            .all(|sent| matches!(sent, Sent::OnLink(_, to, _) if *to == RECEIVER))
    );
    Ok(())
}

#[test]
fn reg_ngp_gets_reg1_on_that_link_only_and_once() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    sender.housekeeping(0, &mut io);
    io.take_sent();
    let (one, two) = (link(&sender, 1)?, link(&sender, 2)?);
    sender.handle_link_datagram(100, two, &answer(PacketType::RegNgp), &mut io);
    sender.handle_link_datagram(100, one, &answer(PacketType::RegNgp), &mut io);
    assert_eq!(
        io.take_sent(),
        vec![Sent::OnLink(
            two,
            RECEIVER,
            registration(PacketType::Reg1, &proposed_id())
        )],
        "one REG1 pending at a time"
    );
    Ok(())
}

#[test]
fn the_reg2_answer_is_adopted_and_sent_on_every_link() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    sender.housekeeping(0, &mut io);
    let one = link(&sender, 1)?;
    sender.handle_link_datagram(0, one, &answer(PacketType::RegNgp), &mut io);
    io.take_sent();
    sender.handle_link_datagram(0, one, &registration(PacketType::Reg2, &full_id()), &mut io);
    assert_eq!(sender.group_id(), &full_id());
    let reg2 = registration(PacketType::Reg2, &full_id());
    for k in [1, 2] {
        assert_eq!(io.on_link(link(&sender, k)?), vec![reg2.clone()]);
    }
    assert!(
        io.events
            .contains(&SenderEvent::GroupRegistered { source: source(1) })
    );
    Ok(())
}

#[test]
fn a_reg2_with_someone_elses_half_is_ignored() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    sender.housekeeping(0, &mut io);
    let one = link(&sender, 1)?;
    sender.handle_link_datagram(0, one, &answer(PacketType::RegNgp), &mut io);
    io.take_sent();
    let mut foreign = full_id();
    foreign[0] ^= 0xFF;
    sender.handle_link_datagram(0, one, &registration(PacketType::Reg2, &foreign), &mut io);
    assert_eq!(sender.group_id(), &proposed_id());
    assert!(io.take_sent().is_empty());
    assert!(
        io.events
            .contains(&SenderEvent::GroupIdMismatch { source: source(1) })
    );
    Ok(())
}

#[test]
fn a_reg2_on_a_link_that_sent_no_reg1_is_ignored() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    sender.housekeeping(0, &mut io);
    sender.handle_link_datagram(0, link(&sender, 1)?, &answer(PacketType::RegNgp), &mut io);
    io.take_sent();
    sender.handle_link_datagram(
        0,
        link(&sender, 2)?,
        &registration(PacketType::Reg2, &full_id()),
        &mut io,
    );
    assert_eq!(sender.group_id(), &proposed_id());
    Ok(())
}

#[test]
fn reg3_makes_a_link_usable() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    assert!(sender.has_connected());
    assert!(sender.links(0).all(|link| link.usable));
    Ok(())
}

#[test]
fn reg_ngp_is_ignored_once_a_link_works_and_for_4_s_after_registering() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let one = link(&sender, 1)?;
    sender.handle_link_datagram(500, one, &answer(PacketType::RegNgp), &mut io);
    assert!(io.take_sent().is_empty(), "links are active");
    Ok(())
}

#[test]
fn a_pending_reg1_is_sent_again_each_second_then_expires() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    sender.housekeeping(0, &mut io);
    let (one, two) = (link(&sender, 1)?, link(&sender, 2)?);
    sender.handle_link_datagram(0, one, &answer(PacketType::RegNgp), &mut io);
    io.take_sent();
    sender.housekeeping(1_000, &mut io);
    assert_eq!(
        io.take_sent(),
        vec![Sent::OnLink(
            one,
            RECEIVER,
            registration(PacketType::Reg1, &proposed_id())
        )],
        "REG1 again on the pending link, nothing on the others"
    );
    sender.housekeeping(5_000, &mut io);
    let sent = io.take_sent();
    assert!(
        sent.contains(&Sent::OnLink(
            two,
            RECEIVER,
            registration(PacketType::Reg2, &proposed_id())
        )),
        "after 4 s the REG1 expires and every link sends REG2 again"
    );
    Ok(())
}

#[test]
fn reg_ngp_and_reg2_do_not_count_as_activity() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let one = link(&sender, 1)?;
    for second in 1..=5 {
        sender.handle_link_datagram(second * 1_000, one, &answer(PacketType::RegNgp), &mut io);
        sender.handle_link_datagram(
            second * 1_000,
            one,
            &registration(PacketType::Reg2, &full_id()),
            &mut io,
        );
    }
    assert!(sender.links(5_000).all(|link| !link.usable));
    Ok(())
}

#[test]
fn reg3_of_any_length_establishes() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    let mut long_reg3 = answer(PacketType::Reg3);
    long_reg3.extend([0; 30]);
    sender.handle_link_datagram(0, link(&sender, 1)?, &long_reg3, &mut io);
    assert!(sender.has_connected());
    assert!(io.to_srt().is_empty(), "never forwarded to SRT");
    Ok(())
}
