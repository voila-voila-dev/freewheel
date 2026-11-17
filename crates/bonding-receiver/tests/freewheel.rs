//! The freewheel receiver on its own: joining, acknowledging, following a link's address,
//! refusing strangers, forgetting silent links.

use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::freewheel::{
    CloseReason, Data, Header, MAX_DATAGRAM_LEN, Packet, TrafficClass,
};
use bonding_receiver::freewheel::{
    ACK_EVERY_PACKETS, ACK_MAX_DELAY_US, GroupHandle, IoFailure, LINK_TIMEOUT_US, Receiver,
    ReceiverIo,
};

#[derive(Default)]
struct RecordingIo {
    to_links: Vec<(SocketAddrV4, Vec<u8>)>,
    to_srt: Vec<Vec<u8>>,
}

impl ReceiverIo for RecordingIo {
    fn send_to_link(&mut self, link: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.to_links.push((link, datagram.to_vec()));
        Ok(())
    }
    fn open_srt(&mut self, _: GroupHandle) -> Result<(), IoFailure> {
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure> {
        self.to_srt.push(datagram.to_vec());
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure> {
        bytes.fill(3);
        Ok(())
    }
}

impl RecordingIo {
    /// What was sent to links since last time, decoded.
    fn take_packets(&mut self) -> Vec<(SocketAddrV4, Vec<u8>)> {
        std::mem::take(&mut self.to_links)
    }
}

const LINK: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 1, 0, 2), 40_000);
const MOVED: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 9, 0, 2), 41_000);
const HEADER: Header = Header {
    session_id: 77,
    link_id: 0,
};

fn bytes(packet: &Packet<'_>) -> Vec<u8> {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let len = packet.encode(&mut out).unwrap_or(0);
    out[..len].to_vec()
}

fn data(sequence: u32, sent_at_us: u32) -> Vec<u8> {
    bytes(&Packet::Data(Data {
        header: HEADER,
        class: TrafficClass::Live,
        sequence,
        sent_at_us,
        payload: &[0x11; 100],
    }))
}

fn joined() -> (Receiver, RecordingIo) {
    let mut receiver = Receiver::new();
    let mut io = RecordingIo::default();
    receiver.handle_link_datagram(0, LINK, &bytes(&Packet::Join(HEADER)), &mut io);
    (receiver, io)
}

#[test]
fn a_join_opens_a_session_and_is_accepted() {
    let (receiver, mut io) = joined();
    assert_eq!(receiver.session_count(), 1);
    assert_eq!(io.take_packets(), [(LINK, bytes(&Packet::Accept(HEADER)))]);
}

#[test]
fn data_reaches_srt_and_is_acknowledged_every_few_packets() {
    let (mut receiver, mut io) = joined();
    io.take_packets();
    for sequence in 0..ACK_EVERY_PACKETS {
        receiver.handle_link_datagram(1_000, LINK, &data(sequence, 500 + sequence), &mut io);
    }
    assert_eq!(io.to_srt.len(), ACK_EVERY_PACKETS as usize);
    let sent = io.take_packets();
    assert_eq!(sent.len(), 1);
    let Ok(Packet::Ack(ack)) = Packet::decode(&sent[0].1) else {
        unreachable!("an ACK")
    };
    assert_eq!(ack.timing.largest_sequence, ACK_EVERY_PACKETS - 1);
    assert_eq!(ack.timing.echoed_sent_at_us, 500 + ACK_EVERY_PACKETS - 1);
    assert_eq!(ack.timing.ack_delay_us, 0);
    assert_eq!(ack.timing.received_bytes, ACK_EVERY_PACKETS * 120);
}

#[test]
fn a_lone_packet_is_acknowledged_after_the_delay_with_its_hold_time() {
    let (mut receiver, mut io) = joined();
    io.take_packets();
    receiver.handle_link_datagram(1_000, LINK, &data(0, 9), &mut io);
    receiver.housekeeping(1_000 + ACK_MAX_DELAY_US - 1, &mut io);
    assert!(io.take_packets().is_empty());
    receiver.housekeeping(1_000 + ACK_MAX_DELAY_US, &mut io);
    let sent = io.take_packets();
    let Ok(Packet::Ack(ack)) = Packet::decode(&sent[0].1) else {
        unreachable!("an ACK")
    };
    assert_eq!(u64::from(ack.timing.ack_delay_us), ACK_MAX_DELAY_US);
}

#[test]
fn keepalives_are_echoed_at_once() {
    let (mut receiver, mut io) = joined();
    io.take_packets();
    let keepalive = Packet::Keepalive {
        header: HEADER,
        sent_at_us: 1_234,
    };
    receiver.handle_link_datagram(5, LINK, &bytes(&keepalive), &mut io);
    let sent = io.take_packets();
    let Ok(Packet::Ack(ack)) = Packet::decode(&sent[0].1) else {
        unreachable!("an ACK")
    };
    assert_eq!(ack.timing.echoed_sent_at_us, 1_234);
}

#[test]
fn a_link_that_changes_address_is_followed() {
    let (mut receiver, mut io) = joined();
    for sequence in 0..ACK_EVERY_PACKETS {
        receiver.handle_link_datagram(10, MOVED, &data(sequence, 0), &mut io);
    }
    let sent = io.take_packets();
    assert_eq!(sent.last().map(|(to, _)| *to), Some(MOVED));
}

#[test]
fn strangers_are_told_to_join() {
    let mut receiver = Receiver::new();
    let mut io = RecordingIo::default();
    receiver.handle_link_datagram(0, LINK, &data(0, 0), &mut io);
    assert!(io.to_srt.is_empty());
    let refused = Packet::Close {
        header: HEADER,
        reason: CloseReason::Refused,
    };
    assert_eq!(io.take_packets(), [(LINK, bytes(&refused))]);
}

#[test]
fn silent_links_are_forgotten() {
    let (mut receiver, mut io) = joined();
    receiver.housekeeping(LINK_TIMEOUT_US, &mut io);
    io.take_packets();
    receiver.handle_link_datagram(LINK_TIMEOUT_US, LINK, &data(0, 0), &mut io);
    assert!(io.to_srt.is_empty());
}
