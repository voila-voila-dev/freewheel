//! How SRTLA ACKs, SRT ACKs and SRT NAKs move windows and packets in flight.

mod support;

use bonding_sender::srtla::{
    LinkHandle, Sender, WINDOW_DEFAULT_MILLIPACKETS, WINDOW_MAX_MILLIPACKETS,
    WINDOW_MIN_MILLIPACKETS,
};
use support::{
    CALLER, FakeIo, TestResult, connect, data, link, loss_range, sender_on, srt_ack, srt_nak,
    srtla_ack,
};

fn status(sender: &Sender, link: LinkHandle) -> (u32, u32) {
    sender
        .links(0)
        .find(|status| status.handle == link)
        .map_or((0, 0), |status| {
            (status.window_millipackets, status.in_flight_packets)
        })
}

/// Two connected links; numbers 100.. alternate between link 2 and link 1.
fn two_links_with(
    packets: u32,
) -> Result<(Sender, FakeIo, LinkHandle, LinkHandle), Box<dyn std::error::Error>> {
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2], &mut io)?;
    connect(&mut sender, &mut io, 0)?;
    for number in 100..100 + packets {
        sender.handle_srt_datagram(0, CALLER, &data(number), &mut io);
    }
    let (one, two) = (link(&sender, 1)?, link(&sender, 2)?);
    io.take_sent();
    Ok((sender, io, one, two))
}

#[test]
fn an_srtla_ack_frees_its_packet_and_grows_every_link_by_one() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(4)?;
    sender.handle_link_datagram(0, one, &srtla_ack(&[101]), &mut io);
    assert_eq!(status(&sender, one), (WINDOW_DEFAULT_MILLIPACKETS + 1, 1));
    assert_eq!(status(&sender, two), (WINDOW_DEFAULT_MILLIPACKETS + 1, 2));
    assert!(io.take_sent().is_empty(), "SRTLA ACKs never reach SRT");
    Ok(())
}

#[test]
fn an_srtla_ack_is_matched_on_any_link_not_only_the_one_it_came_on() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(4)?;
    sender.handle_link_datagram(0, one, &srtla_ack(&[100]), &mut io);
    assert_eq!(status(&sender, two).1, 1, "100 went on link 2");
    Ok(())
}

#[test]
fn a_link_over_its_window_grows_by_30_per_ack() -> TestResult {
    let (mut sender, mut io, _, two) = two_links_with(60)?;
    sender.handle_link_datagram(0, two, &srtla_ack(&[100]), &mut io);
    assert_eq!(status(&sender, two), (WINDOW_DEFAULT_MILLIPACKETS + 30, 29));
    Ok(())
}

#[test]
fn windows_stop_at_the_maximum() -> TestResult {
    let (mut sender, mut io, one, _) = two_links_with(0)?;
    let unknown: Vec<u32> = (0..10).collect();
    for _ in 0..5_000 {
        sender.handle_link_datagram(0, one, &srtla_ack(&unknown), &mut io);
    }
    assert_eq!(status(&sender, one).0, WINDOW_MAX_MILLIPACKETS);
    Ok(())
}

#[test]
fn an_srt_ack_recounts_in_flight_on_every_link() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(10)?;
    sender.handle_link_datagram(0, one, &srt_ack(106), &mut io);
    assert_eq!(status(&sender, one).1, 2, "107, 109 left on link 1");
    assert_eq!(status(&sender, two).1, 2, "106, 108 left on link 2");
    assert_eq!(io.to_srt(), vec![srt_ack(106)], "forwarded to the caller");
    Ok(())
}

#[test]
fn a_nak_shrinks_the_window_of_the_link_that_carried_the_packet() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(4)?;
    sender.handle_link_datagram(0, one, &srt_nak(&[100]), &mut io);
    assert_eq!(status(&sender, two).0, WINDOW_DEFAULT_MILLIPACKETS - 100);
    assert_eq!(status(&sender, one).0, WINDOW_DEFAULT_MILLIPACKETS);
    assert_eq!(
        status(&sender, two).1,
        2,
        "a NAK does not change what is in flight"
    );
    assert_eq!(io.to_srt(), vec![srt_nak(&[100])]);
    Ok(())
}

#[test]
fn a_nak_range_counts_each_packet_once() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(10)?;
    sender.handle_link_datagram(0, one, &srt_nak(&loss_range(100, 104)), &mut io);
    assert_eq!(
        status(&sender, two).0,
        WINDOW_DEFAULT_MILLIPACKETS - 300,
        "100, 102, 104"
    );
    assert_eq!(
        status(&sender, one).0,
        WINDOW_DEFAULT_MILLIPACKETS - 200,
        "101, 103"
    );
    sender.handle_link_datagram(0, one, &srt_nak(&loss_range(100, 104)), &mut io);
    assert_eq!(
        status(&sender, two).0,
        WINDOW_DEFAULT_MILLIPACKETS - 300,
        "the same loss again finds nothing: the copies were forgotten"
    );
    Ok(())
}

#[test]
fn a_retransmitted_packet_is_blamed_on_the_first_listed_link_that_logged_it() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(3)?;
    sender.handle_srt_datagram(0, CALLER, &data(100), &mut io);
    assert_eq!(
        io.data_links(),
        vec![one],
        "the retransmission of 100 goes on link 1"
    );
    sender.handle_link_datagram(0, one, &srt_nak(&[100]), &mut io);
    assert_eq!(status(&sender, two).0, WINDOW_DEFAULT_MILLIPACKETS - 100);
    assert_eq!(status(&sender, one).0, WINDOW_DEFAULT_MILLIPACKETS);
    sender.handle_link_datagram(0, one, &srt_nak(&[100]), &mut io);
    assert_eq!(
        status(&sender, one).0,
        WINDOW_DEFAULT_MILLIPACKETS - 100,
        "the next NAK of 100 finds the other copy"
    );
    Ok(())
}

#[test]
fn a_huge_nak_range_is_bounded_by_the_logs() -> TestResult {
    let (mut sender, mut io, one, two) = two_links_with(400)?;
    sender.handle_link_datagram(0, one, &srt_nak(&[0x8000_0000, 0x7FFF_FFFF]), &mut io);
    assert_eq!(status(&sender, one).0, WINDOW_MIN_MILLIPACKETS);
    assert_eq!(status(&sender, two).0, WINDOW_MIN_MILLIPACKETS);
    Ok(())
}
