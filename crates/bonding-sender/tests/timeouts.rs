//! Link failure after 4 s of silence, and giving up after 10 s without any link.

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_sender::srtla::{HousekeepingOutcome, SenderEvent, WINDOW_MIN_MILLIPACKETS};
use support::{
    CALLER, FALLBACK_RECEIVER, FakeIo, RECEIVER, Sent, TestResult, answer, connect, data, full_id,
    link, registration, sender_on, source,
};

#[test]
fn a_silent_link_fails_after_more_than_4_s_and_re_registers_with_reg2() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let (one, two) = (link(&sender, 1)?, link(&sender, 2)?);
    sender.handle_srt_datagram(0, CALLER, &data(1), &mut io);
    for second in 1..=5 {
        sender.handle_link_datagram(second * 1_000, one, &answer(PacketType::Keepalive), &mut io);
    }
    sender.housekeeping(4_000, &mut io);
    assert!(
        sender.links(4_999).all(|status| status.usable),
        "4 s is not more than 4 s"
    );
    io.take_sent();
    sender.housekeeping(5_000, &mut io);
    assert!(
        io.events
            .contains(&SenderEvent::LinkFailed { source: source(2) })
    );
    assert!(io.take_sent().contains(&Sent::OnLink(
        two,
        RECEIVER,
        registration(PacketType::Reg2, &full_id())
    )));
    let status = sender.links(5_000).find(|status| status.handle == two);
    assert_eq!(
        status.map(|status| (
            status.usable,
            status.window_millipackets,
            status.in_flight_packets
        )),
        Some((false, WINDOW_MIN_MILLIPACKETS, 0))
    );
    sender.handle_srt_datagram(5_000, CALLER, &data(2), &mut io);
    assert_eq!(
        io.data_links(),
        vec![one],
        "the failed link carries nothing"
    );
    Ok(())
}

#[test]
fn a_returning_link_restarts_from_the_minimum_window() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    sender.housekeeping(5_000, &mut io);
    let one = link(&sender, 1)?;
    sender.handle_link_datagram(6_000, one, &answer(PacketType::Reg3), &mut io);
    let status = sender.links(6_000).next();
    assert_eq!(
        status.map(|status| (status.usable, status.window_millipackets)),
        Some((true, WINDOW_MIN_MILLIPACKETS))
    );
    Ok(())
}

#[test]
fn the_sender_gives_up_after_10_s_without_links_once_connected() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let mut last = HousekeepingOutcome::Running;
    let mut gave_up_at_ms = None;
    for second in 1..=20 {
        last = sender.housekeeping(second * 1_000, &mut io);
        if last == HousekeepingOutcome::GaveUp {
            gave_up_at_ms = Some(second * 1_000);
            break;
        }
    }
    assert_eq!(last, HousekeepingOutcome::GaveUp);
    assert_eq!(
        gave_up_at_ms,
        Some(16_000),
        "failed at 5 s, more than 10 s later"
    );
    assert!(io.events.contains(&SenderEvent::NoLinkAvailable));
    assert!(io.events.contains(&SenderEvent::GaveUp {
        had_connected: true
    }));
    Ok(())
}

#[test]
fn before_ever_connecting_it_tries_the_next_receiver_address() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    let mut outcomes = Vec::new();
    for second in 0..=25 {
        outcomes.push(sender.housekeeping(second * 1_000, &mut io));
    }
    assert!(io.events.contains(&SenderEvent::TryingReceiver {
        receiver: FALLBACK_RECEIVER
    }));
    assert_eq!(sender.receiver(), Some(FALLBACK_RECEIVER));
    assert!(
        !io.events.contains(&SenderEvent::NoLinkAvailable),
        "no warning before the first connection"
    );
    assert_eq!(
        outcomes.last(),
        Some(&HousekeepingOutcome::GaveUp),
        "no third address"
    );
    Ok(())
}

#[test]
fn housekeeping_runs_at_most_once_a_second() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    sender.housekeeping(0, &mut io);
    io.take_sent();
    sender.housekeeping(999, &mut io);
    assert!(io.take_sent().is_empty());
    sender.housekeeping(1_000, &mut io);
    assert_eq!(io.take_sent().len(), 1, "REG2 again");
    Ok(())
}

#[test]
fn a_link_that_cannot_bind_is_retried_every_housekeeping() -> TestResult {
    let mut io = FakeIo {
        unbindable: vec![source(2)],
        ..FakeIo::default()
    };
    let mut sender = sender_on(&[1, 2], &mut io)?;
    assert!(
        io.events
            .contains(&SenderEvent::LinkNotOpened { source: source(2) })
    );
    io.unbindable.clear();
    sender.housekeeping(0, &mut io);
    assert!(sender.links(0).all(|status| status.open));
    Ok(())
}
