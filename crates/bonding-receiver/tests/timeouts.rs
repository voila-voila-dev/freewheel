//! Forgetting silent links (10 s) and empty groups (10 s), checked every 3 s.

mod support;

use bonding_receiver::srtla::{GroupRemoval, Receiver, ReceiverEvent};
use support::{FakeIo, TestResult, data, link, reg1, register};

#[test]
fn a_link_silent_for_more_than_ten_seconds_is_forgotten() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1), link(2)])?;
    for now_s in 0..=20 {
        receiver.handle_link_datagram(now_s, link(1), &data(1), &mut io);
        receiver.housekeeping(now_s, &mut io);
        let expected = if now_s < 12 { 2 } else { 1 };
        assert_eq!(receiver.links(group).count(), expected, "at {now_s} s");
    }
    assert!(io.events.contains(&ReceiverEvent::LinkTimedOut {
        group,
        link: link(2)
    }));
    Ok(())
}

#[test]
fn cleanup_runs_at_most_every_three_seconds() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    receiver.housekeeping(10, &mut io);
    receiver.housekeeping(11, &mut io);
    receiver.housekeeping(12, &mut io);
    assert_eq!(
        receiver.links(group).count(),
        1,
        "11 and 12 s fell inside the 3 s period"
    );
    receiver.housekeeping(13, &mut io);
    assert_eq!(receiver.group_count(), 0);
    Ok(())
}

#[test]
fn a_group_without_links_goes_after_ten_seconds_and_closes_its_srt_socket() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1)])?;
    receiver.handle_link_datagram(0, link(1), &data(1), &mut io);
    receiver.handle_link_datagram(0, link(3), &reg1(2), &mut io);
    receiver.housekeeping(10, &mut io);
    assert_eq!(receiver.group_count(), 2);
    receiver.housekeeping(13, &mut io);
    assert_eq!(receiver.group_count(), 0);
    assert_eq!(io.closed, [group], "only the group that opened one");
    assert!(io.events.contains(&ReceiverEvent::GroupRemoved {
        group,
        reason: GroupRemoval::NoLinks
    }));
    Ok(())
}
