//! Reloading the link list (`SIGHUP`): new addresses join, missing ones leave, others keep
//! their state.

mod support;

use bonding_sender::server::parse_source_list;
use bonding_sender::srtla::{SenderEvent, WINDOW_DEFAULT_MILLIPACKETS};
use std::net::Ipv4Addr;
use support::{CALLER, FakeIo, TestResult, connect, data, link, sender_on, source, srt_nak};

#[test]
fn a_new_address_starts_at_the_default_window_and_is_listed_first() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let added = sender.reload_sources(&[source(1), source(2), source(3)], &mut io);
    assert_eq!(added, 1);
    let first = sender.links(0).next();
    assert_eq!(
        first.map(|status| (status.source, status.open, status.window_millipackets)),
        Some((source(3), false, WINDOW_DEFAULT_MILLIPACKETS))
    );
    sender.housekeeping(1_000, &mut io);
    assert!(
        sender.links(1_000).all(|status| status.open),
        "opened by housekeeping"
    );
    Ok(())
}

#[test]
fn a_kept_address_keeps_its_state() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    sender.handle_srt_datagram(0, CALLER, &data(5), &mut io);
    let two = link(&sender, 2)?;
    sender.handle_link_datagram(0, two, &srt_nak(&[5]), &mut io);
    sender.reload_sources(&[source(2), source(1)], &mut io);
    assert_eq!(link(&sender, 2)?, two);
    let window = sender
        .links(0)
        .find(|status| status.handle == two)
        .map(|status| status.window_millipackets);
    assert_eq!(window, Some(WINDOW_DEFAULT_MILLIPACKETS - 100));
    Ok(())
}

#[test]
fn a_missing_address_is_closed_at_once() -> TestResult {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    let two = link(&sender, 2)?;
    sender.reload_sources(&[source(1)], &mut io);
    assert_eq!(io.closed, vec![two]);
    assert!(
        io.events
            .contains(&SenderEvent::LinkRemoved { source: source(2) })
    );
    for number in 0..4 {
        sender.handle_srt_datagram(0, CALLER, &data(number), &mut io);
    }
    assert!(io.data_links().iter().all(|link| *link != two));
    Ok(())
}

#[test]
fn the_source_list_reads_like_inet_addr() {
    let text = b"10.1.0.2\n\n10.2.0.2 modem two\r\n 10.3.0.2\nnot an address\n255.255.255.255\n10.1.0.2\n10.4.0.2";
    assert_eq!(
        parse_source_list(text),
        vec![
            Ipv4Addr::new(10, 1, 0, 2),
            Ipv4Addr::new(10, 2, 0, 2),
            Ipv4Addr::new(10, 1, 0, 2),
            Ipv4Addr::new(10, 4, 0, 2),
        ],
        "leading blanks and broadcast refused; duplicates are merged by the sender"
    );
}

#[test]
fn duplicate_addresses_make_one_link() -> TestResult {
    let mut io = FakeIo::default();
    let sender = sender_on(&[1, 1, 2], &mut io)?;
    assert_eq!(sender.links(0).count(), 2);
    Ok(())
}
