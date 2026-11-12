//! Registration: REG1 creates a group, REG2 adds links, REG3 confirms each one.

mod support;

use bonding_protocol::srtla::{HALF_ID_LEN, PacketType};
use bonding_receiver::srtla::{MAX_GROUPS, MAX_LINKS_PER_GROUP, Receiver, ReceiverEvent, Refusal};
use support::{
    FakeIo, Operation, Sent, TestResult, answer, id_in, link, link_of, reg1, reg2, register,
};

#[test]
fn reg1_creates_a_group_keeping_the_sender_half_of_the_id() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    receiver.handle_link_datagram(0, link(1), &reg1(0x42), &mut io);
    let answers = io.to_link(link(1));
    let id = answers
        .first()
        .and_then(|packet| id_in(packet))
        .ok_or("no REG2")?;
    assert_eq!(answers.len(), 1);
    assert!(id[..HALF_ID_LEN].iter().all(|byte| *byte == 0x42));
    assert!(
        id[HALF_ID_LEN..].iter().all(|byte| *byte == 1),
        "receiver half is random"
    );
    assert_eq!(receiver.group_count(), 1);
    assert_eq!(
        receiver.group_of(link(1)),
        None,
        "REG1 alone registers no link"
    );
    Ok(())
}

#[test]
fn every_link_gets_its_own_reg3() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    receiver.handle_link_datagram(0, link(1), &reg1(7), &mut io);
    let id = io
        .to_link(link(1))
        .first()
        .and_then(|p| id_in(p))
        .ok_or("no REG2")?;
    io.take_sent();
    for k in 1..=3 {
        receiver.handle_link_datagram(0, link(k), &reg2(&id), &mut io);
    }
    let expected: Vec<Sent> = (1..=3)
        .map(|k| Sent::ToLink(link(k), answer(PacketType::Reg3)))
        .collect();
    assert_eq!(io.sent, expected);
    let group = receiver.group_of(link(1)).ok_or("no group")?;
    assert_eq!(
        receiver.links(group).collect::<Vec<_>>(),
        [link(3), link(2), link(1)]
    );
    Ok(())
}

#[test]
fn an_unknown_id_gets_reg_ngp() {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    receiver.handle_link_datagram(0, link(1), &reg2(&[9; 256]), &mut io);
    assert_eq!(io.sent, [Sent::ToLink(link(1), answer(PacketType::RegNgp))]);
    assert_eq!(receiver.group_count(), 0);
}

#[test]
fn re_registering_a_link_is_harmless_and_is_not_activity() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, id) = register(&mut receiver, &mut io, 0, 1, &[link(1), link(2)])?;
    receiver.handle_link_datagram(9, link(1), &reg2(&id), &mut io);
    assert_eq!(
        io.take_sent(),
        [Sent::ToLink(link(1), answer(PacketType::Reg3))]
    );
    assert_eq!(receiver.links(group).count(), 2);
    assert!(io.events.contains(&ReceiverEvent::LinkRegistered {
        group,
        link: link(1),
        again: true
    }));
    receiver.handle_link_datagram(9, link(2), &answer(PacketType::Keepalive), &mut io);
    receiver.housekeeping(11, &mut io);
    assert_eq!(
        receiver.links(group).collect::<Vec<_>>(),
        [link(2)],
        "REG2 kept nothing alive"
    );
    Ok(())
}

#[test]
fn an_address_cannot_create_two_groups() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    receiver.handle_link_datagram(0, link(1), &reg1(1), &mut io);
    io.take_sent();
    receiver.handle_link_datagram(0, link(1), &reg1(2), &mut io);
    assert_eq!(
        io.take_sent(),
        [Sent::ToLink(link(1), answer(PacketType::RegErr))]
    );
    register(&mut receiver, &mut io, 0, 3, &[link(2)])?;
    receiver.handle_link_datagram(0, link(2), &reg1(4), &mut io);
    assert_eq!(
        io.take_sent(),
        [Sent::ToLink(link(2), answer(PacketType::RegErr))]
    );
    assert_eq!(receiver.group_count(), 2);
    Ok(())
}

#[test]
fn a_link_of_one_group_cannot_join_another() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (first, _) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    let (_, other_id) = register(&mut receiver, &mut io, 0, 2, &[link_of(2, 1)])?;
    receiver.handle_link_datagram(0, link(1), &reg2(&other_id), &mut io);
    assert_eq!(
        io.take_sent(),
        [Sent::ToLink(link(1), answer(PacketType::RegErr))]
    );
    assert_eq!(receiver.group_of(link(1)), Some(first));
    Ok(())
}

#[test]
fn the_ninth_link_is_refused() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let links: Vec<_> = (1..=8).map(link).collect();
    let (group, id) = register(&mut receiver, &mut io, 0, 1, &links)?;
    receiver.handle_link_datagram(0, link(9), &reg2(&id), &mut io);
    assert_eq!(
        io.take_sent(),
        [Sent::ToLink(link(9), answer(PacketType::RegErr))]
    );
    assert_eq!(receiver.links(group).count(), MAX_LINKS_PER_GROUP);
    assert!(io.events.contains(&ReceiverEvent::LinkRefused {
        link: link(9),
        reason: Refusal::GroupFull
    }));
    Ok(())
}

#[test]
fn the_201st_group_is_refused_until_others_expire() {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let flooder =
        |port: u16| std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(10, 9, 0, 2), port);
    for port in 0..u16::try_from(MAX_GROUPS).unwrap_or_default() {
        receiver.handle_link_datagram(0, flooder(1_000 + port), &reg1(1), &mut io);
    }
    assert_eq!(receiver.group_count(), MAX_GROUPS);
    io.take_sent();
    receiver.handle_link_datagram(5, link(1), &reg1(2), &mut io);
    assert_eq!(
        io.take_sent(),
        [Sent::ToLink(link(1), answer(PacketType::RegErr))]
    );
    receiver.housekeeping(11, &mut io);
    assert_eq!(receiver.group_count(), 0, "empty groups expire after 10 s");
    receiver.handle_link_datagram(11, link(1), &reg1(2), &mut io);
    assert_eq!(receiver.group_count(), 1);
}

#[test]
fn registration_packets_of_the_wrong_length_are_ignored() {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let mut long = reg1(1);
    long.push(0);
    receiver.handle_link_datagram(0, link(1), &long, &mut io);
    receiver.handle_link_datagram(0, link(1), &reg1(1)[..200], &mut io);
    assert_eq!(io.sent, []);
    assert_eq!(receiver.group_count(), 0);
}

#[test]
fn a_colliding_random_half_is_drawn_again() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    io.random_bytes = vec![5, 5, 6];
    let (_, first) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    let (_, second) = register(&mut receiver, &mut io, 0, 1, &[link(2)])?;
    assert_eq!(first[HALF_ID_LEN], 5);
    assert_eq!(second[HALF_ID_LEN], 6, "same sender half: the 5 collided");
    Ok(())
}

#[test]
fn without_randomness_reg1_is_refused() {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::failing(&[Operation::Randomness]));
    receiver.handle_link_datagram(0, link(1), &reg1(1), &mut io);
    assert_eq!(io.sent, [Sent::ToLink(link(1), answer(PacketType::RegErr))]);
    assert_eq!(receiver.group_count(), 0);
}

#[test]
fn a_group_whose_reg2_cannot_be_sent_is_not_created() {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::failing(&[Operation::SendToLink]));
    receiver.handle_link_datagram(0, link(1), &reg1(1), &mut io);
    assert_eq!(receiver.group_count(), 0);
    assert!(io.events.contains(&ReceiverEvent::GroupRefused {
        from: link(1),
        reason: Refusal::AnswerNotSent
    }));
}
