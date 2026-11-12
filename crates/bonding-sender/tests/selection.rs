//! Link selection: the best `window / (in_flight + 1)` among usable links, the first on a tie.

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_sender::srtla::{WINDOW_DEFAULT_MILLIPACKETS, WINDOW_MIN_MILLIPACKETS};
use support::{CALLER, FakeIo, TestResult, answer, connect, data, link, sender_on, srt_control};

#[test]
fn nothing_is_sent_before_registration() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    sender.housekeeping(0, &mut io);
    io.take_sent();
    sender.handle_srt_datagram(100, CALLER, &data(1), &mut io);
    assert!(io.take_sent().is_empty(), "dropped: no usable link");
    Ok(())
}

#[test]
fn equal_links_alternate_newest_listed_first() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let (one, two) = (link(&sender, 1)?, link(&sender, 2)?);
    for number in 0..4 {
        sender.handle_srt_datagram(0, CALLER, &data(number), &mut io);
    }
    assert_eq!(
        io.data_links(),
        vec![two, one, two, one],
        "the last line of the file is first in upstream's list"
    );
    Ok(())
}

#[test]
fn in_flight_packets_lower_the_score() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    for number in 0..10 {
        sender.handle_srt_datagram(0, CALLER, &data(number), &mut io);
    }
    for status in sender.links(0) {
        assert_eq!(status.in_flight_packets, 5);
        assert_eq!(status.window_millipackets, WINDOW_DEFAULT_MILLIPACKETS);
    }
    Ok(())
}

#[test]
fn control_packets_are_sent_but_not_counted() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    sender.handle_srt_datagram(0, CALLER, &srt_control(PacketType::SrtHandshake), &mut io);
    assert_eq!(io.take_sent().len(), 1);
    assert!(sender.links(0).all(|link| link.in_flight_packets == 0));
    Ok(())
}

#[test]
fn a_link_that_only_got_reg_err_is_used() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1], &mut io)?;
    let one = link(&sender, 1)?;
    sender.handle_link_datagram(0, one, &answer(PacketType::RegErr), &mut io);
    io.take_sent();
    sender.handle_srt_datagram(0, CALLER, &data(7), &mut io);
    assert_eq!(
        io.data_links(),
        vec![one],
        "REG_ERR counts as hearing from the receiver"
    );
    Ok(())
}

#[test]
fn a_failed_send_disables_the_link_at_once() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let (one, two) = (link(&sender, 1)?, link(&sender, 2)?);
    io.failing_links.push(two);
    sender.handle_srt_datagram(0, CALLER, &data(1), &mut io);
    for number in 2..5 {
        sender.handle_srt_datagram(0, CALLER, &data(number), &mut io);
    }
    assert_eq!(io.data_links(), vec![one, one, one]);
    sender.housekeeping(1_000, &mut io);
    let two_status = sender.links(1_000).find(|status| status.handle == two);
    assert_eq!(
        two_status.map(|status| status.window_millipackets),
        Some(WINDOW_MIN_MILLIPACKETS),
        "reset at the next housekeeping"
    );
    Ok(())
}
