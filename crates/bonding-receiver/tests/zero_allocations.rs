//! The per-packet path allocates nothing: the count does not grow with traffic.

use std::net::{Ipv4Addr, SocketAddrV4};

use alloc_counter::{CountingAllocator, count_allocations};
use bonding_protocol::srtla::{GroupId, ID_LEN, PacketType};
use bonding_receiver::srtla::{GroupHandle, IoFailure, Receiver, ReceiverIo};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Keeps the last group id it saw and counts bytes; never allocates.
#[derive(Default)]
struct CountingIo {
    bytes_sent: usize,
    id: Option<GroupId>,
}

impl ReceiverIo for CountingIo {
    fn send_to_link(&mut self, _: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        if let Some(id) = datagram.get(2..).and_then(|id| GroupId::try_from(id).ok()) {
            self.id = Some(id);
        }
        self.bytes_sent += datagram.len();
        Ok(())
    }
    fn open_srt(&mut self, _: GroupHandle) -> Result<(), IoFailure> {
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure> {
        self.bytes_sent += datagram.len();
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure> {
        bytes.fill(7);
        Ok(())
    }
}

fn link(k: u8) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(10, k, 0, 2), 4_000)
}

fn stream(packets: u32) -> usize {
    let (mut receiver, mut io) = (Receiver::new(), CountingIo::default());
    let mut reg1 = [0u8; 2 + ID_LEN];
    reg1[..2].copy_from_slice(&PacketType::Reg1.raw().to_be_bytes());
    receiver.handle_link_datagram(0, link(1), &reg1, &mut io);
    let mut reg2 = [0u8; 2 + ID_LEN];
    reg2[..2].copy_from_slice(&PacketType::Reg2.raw().to_be_bytes());
    reg2[2..].copy_from_slice(&io.id.unwrap_or([0; ID_LEN]));
    for k in 1..=4 {
        receiver.handle_link_datagram(0, link(k), &reg2, &mut io);
    }
    let group = receiver.group_of(link(1));
    let mut packet = [0u8; 1_316];
    let mut ack = [0u8; 44];
    ack[..2].copy_from_slice(&PacketType::SrtAck.raw().to_be_bytes());
    for number in 0..packets {
        packet[..4].copy_from_slice(&number.to_be_bytes());
        let k = u8::try_from(number % 4).unwrap_or(0) + 1;
        let now_s = u64::from(number / 1_000);
        receiver.handle_link_datagram(now_s, link(k), &packet, &mut io);
        receiver.handle_link_datagram(now_s, link(k), &[0x90, 0x00], &mut io);
        if let Some(group) = group {
            receiver.handle_srt_datagram(group, &ack, &mut io);
        }
        receiver.housekeeping(now_s, &mut io);
    }
    io.bytes_sent
}

#[test]
fn forwarding_allocates_nothing_per_packet() {
    let (_, small) = count_allocations(|| stream(1_000));
    let (_, large) = count_allocations(|| stream(100_000));
    assert_eq!(
        small, large,
        "{small} allocations for 1k packets, {large} for 100k"
    );
}
