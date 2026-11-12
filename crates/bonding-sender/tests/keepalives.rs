//! Keepalives: only on links that carried nothing for more than a second.

mod support;

use bonding_protocol::srtla::PacketType;
use support::{CALLER, FakeIo, TestResult, answer, connect, data, link, sender_on};

#[test]
fn an_idle_link_sends_a_keepalive_every_housekeeping() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let one = link(&sender, 1)?;
    for second in 1..=3 {
        sender.handle_link_datagram(second * 1_000, one, &answer(PacketType::Keepalive), &mut io);
        sender.housekeeping(second * 1_000, &mut io);
        assert_eq!(
            io.take_sent(),
            vec![support::Sent::OnLink(
                one,
                support::RECEIVER,
                answer(PacketType::Keepalive)
            )]
        );
    }
    Ok(())
}

#[test]
fn a_link_carrying_data_sends_none() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    for second in 1..=5 {
        sender.handle_srt_datagram(
            second * 1_000,
            CALLER,
            &data(u32::try_from(second)?),
            &mut io,
        );
        sender.housekeeping(second * 1_000 + 500, &mut io);
    }
    assert!(
        io.take_sent()
            .iter()
            .all(|sent| matches!(sent, support::Sent::OnLink(_, _, bytes) if bytes.len() > 2))
    );
    Ok(())
}

#[test]
fn the_keepalive_echo_keeps_the_link_alive_and_is_not_forwarded() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let one = link(&sender, 1)?;
    sender.handle_srt_datagram(0, CALLER, &data(1), &mut io);
    for second in 1..=20 {
        sender.handle_link_datagram(second * 1_000, one, &answer(PacketType::Keepalive), &mut io);
        sender.housekeeping(second * 1_000, &mut io);
    }
    assert!(sender.links(20_000).all(|status| status.usable));
    assert!(io.to_srt().is_empty());
    Ok(())
}
