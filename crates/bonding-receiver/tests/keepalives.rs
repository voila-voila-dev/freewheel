//! SRTLA keepalives: echoed on their link, never forwarded.

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_receiver::srtla::Receiver;
use support::{FakeIo, Sent, TestResult, answer, link, register};

#[test]
fn a_keepalive_is_echoed_byte_for_byte_and_keeps_its_link_alive() -> TestResult {
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let (group, _) = register(&mut receiver, &mut io, 0, 1, &[link(1), link(2)])?;
    let mut keepalive = answer(PacketType::Keepalive);
    keepalive.extend([1, 2, 3]);
    receiver.handle_link_datagram(8, link(2), &keepalive, &mut io);
    assert_eq!(io.sent, [Sent::ToLink(link(2), keepalive)]);
    assert_eq!(io.opened, [], "never reaches SRT");
    receiver.housekeeping(12, &mut io);
    assert_eq!(receiver.links(group).collect::<Vec<_>>(), [link(2)]);
    Ok(())
}
