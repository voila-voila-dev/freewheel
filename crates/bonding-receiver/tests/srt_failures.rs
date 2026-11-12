//! A failing SRT socket destroys its group, as upstream does (`srt-server-restart`).

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_receiver::srtla::{GroupRemoval, Receiver, ReceiverEvent};
use support::{FakeIo, Operation, TestResult, data, link, register, srt_control};

#[test]
fn a_read_error_destroys_the_group_and_its_links_become_strangers() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    receiver.handle_link_datagram(1, link(1), &data(1), &mut io);
    receiver.handle_srt_failure(group, &mut io);
    assert_eq!(receiver.group_count(), 0);
    assert_eq!(io.closed, [group]);
    assert!(io.events.contains(&ReceiverEvent::GroupRemoved {
        group,
        reason: GroupRemoval::SrtFailed
    }));
    io.take_sent();
    receiver.handle_link_datagram(2, link(1), &data(2), &mut io);
    assert_eq!(io.sent, []);
    Ok(())
}

#[test]
fn a_short_datagram_from_srt_destroys_the_group() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    receiver.handle_link_datagram(1, link(1), &data(1), &mut io);
    receiver.handle_srt_datagram(group, &srt_control(PacketType::SrtAck)[..15], &mut io);
    assert_eq!(receiver.group_count(), 0);
    Ok(())
}

#[test]
fn failing_to_open_or_send_destroys_the_group() -> TestResult {
    for failing in [Operation::OpenSrt, Operation::SendToSrt] {
        let mut io = FakeIo::failing(&[failing]);
        let mut receiver = Receiver::new();
        register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
        receiver.handle_link_datagram(1, link(1), &data(1), &mut io);
        assert_eq!(receiver.group_count(), 0, "{failing:?}");
    }
    Ok(())
}

#[test]
fn a_failed_send_to_a_link_is_reported_and_nothing_else() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    receiver.handle_link_datagram(1, link(1), &data(1), &mut io);
    io.failing.push(Operation::SendToLink);
    receiver.handle_srt_datagram(group, &srt_control(PacketType::SrtAck), &mut io);
    assert_eq!(receiver.group_count(), 1);
    assert!(
        io.events
            .contains(&ReceiverEvent::SendToLinkFailed { link: link(1) })
    );
    Ok(())
}
