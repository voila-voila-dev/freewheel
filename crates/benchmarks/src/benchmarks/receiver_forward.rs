//! The receiver's per-packet work: find the link, log the number for the SRTLA ACK, forward.

use crate::Benchmark;
use bonding_protocol::srtla::{GroupId, ID_LEN, PacketType, TYPE_LEN};
use bonding_receiver::srtla::{GroupHandle, IoFailure, Receiver, ReceiverIo};
use std::hint::black_box;
use std::net::{Ipv4Addr, SocketAddrV4};

/// `receiver_forward`: a registered 4-link group, data packets spread over its links, one
/// SRT ACK (sent back on every link) every 10 packets.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "receiver_forward",
    run,
};

const LINKS: u8 = 4;
const DATA_PACKET_BYTES: usize = 1_316;
const PACKETS_PER_SRT_ACK: u32 = 10;

/// Counts bytes instead of sending them, and keeps the group id of the last REG2 answer.
#[derive(Default)]
struct CountingIo {
    bytes: u64,
    id: Option<GroupId>,
}

impl ReceiverIo for CountingIo {
    fn send_to_link(&mut self, _: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        if let Some(id) = datagram
            .get(TYPE_LEN..)
            .and_then(|id| GroupId::try_from(id).ok())
        {
            self.id = Some(id);
        }
        self.bytes += datagram.len() as u64;
        Ok(())
    }
    fn open_srt(&mut self, _: GroupHandle) -> Result<(), IoFailure> {
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure> {
        self.bytes += black_box(datagram).len() as u64;
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure> {
        bytes.fill(0x5A);
        Ok(())
    }
}

fn link(k: u8) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(10, k, 0, 2), 40_000)
}

fn registration(kind: PacketType, id: GroupId) -> [u8; TYPE_LEN + ID_LEN] {
    let mut packet = [0u8; TYPE_LEN + ID_LEN];
    let (header, body) = packet.split_at_mut(TYPE_LEN);
    header.copy_from_slice(&kind.raw().to_be_bytes());
    body.copy_from_slice(&id);
    packet
}

fn run(packets: u64) -> u64 {
    let (mut receiver, mut io) = (Receiver::new(), CountingIo::default());
    receiver.handle_link_datagram(
        0,
        link(1),
        &registration(PacketType::Reg1, [1; ID_LEN]),
        &mut io,
    );
    let id = io.id.unwrap_or([0; ID_LEN]);
    for k in 1..=LINKS {
        receiver.handle_link_datagram(0, link(k), &registration(PacketType::Reg2, id), &mut io);
    }
    let Some(group) = receiver.group_of(link(1)) else {
        return 0;
    };
    let mut data = [0u8; DATA_PACKET_BYTES];
    let mut srt_ack = [0u8; 44];
    srt_ack[..TYPE_LEN].copy_from_slice(&PacketType::SrtAck.raw().to_be_bytes());
    for index in 0..packets {
        let number = u32::try_from(index & 0x7FFF_FFFF).unwrap_or(0);
        data[..4].copy_from_slice(&number.to_be_bytes());
        let k = u8::try_from(index % u64::from(LINKS)).unwrap_or(0) + 1;
        receiver.handle_link_datagram(0, black_box(link(k)), black_box(&data), &mut io);
        if number % PACKETS_PER_SRT_ACK == 0 {
            receiver.handle_srt_datagram(group, black_box(&srt_ack), &mut io);
        }
    }
    io.bytes
}
