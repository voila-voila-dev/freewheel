//! The sender's per-packet work: choose a link, send, log the number; read ACKs and NAKs back.

use crate::Benchmark;
use bonding_protocol::srtla::{HALF_ID_LEN, ID_LEN, PacketType, TYPE_LEN};
use bonding_sender::srtla::{IoFailure, LinkHandle, Sender, SenderIo};
use std::hint::black_box;
use std::net::{Ipv4Addr, SocketAddrV4};

/// `sender_forward`: a registered 4-link sender, data packets from the SRT caller, one SRTLA ACK
/// of 10 numbers and one SRT ACK every 10 packets, one NAK of 3 every 100.
pub const BENCHMARK: Benchmark = Benchmark {
    name: "sender_forward",
    run,
};

const LINKS: u8 = 4;
const DATA_PACKET_BYTES: usize = 1_316;
const PACKETS_PER_ACK: u32 = 10;
const PACKETS_PER_NAK: u32 = 100;
const RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), 5_000);
const CALLER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7_001);

/// Counts bytes instead of sending them.
#[derive(Default)]
struct CountingIo {
    bytes: u64,
}

impl SenderIo for CountingIo {
    fn open_link(&mut self, _: LinkHandle, _: Ipv4Addr) -> Result<(), IoFailure> {
        Ok(())
    }
    fn close_link(&mut self, _: LinkHandle) {}
    fn send_on_link(
        &mut self,
        _: LinkHandle,
        _: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), IoFailure> {
        self.bytes += black_box(datagram).len() as u64;
        Ok(())
    }
    fn send_to_srt(&mut self, _: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.bytes += black_box(datagram).len() as u64;
        Ok(())
    }
}

fn control(kind: PacketType, len: usize) -> Vec<u8> {
    let mut packet = vec![0u8; len];
    packet[..TYPE_LEN].copy_from_slice(&kind.raw().to_be_bytes());
    packet
}

fn run(packets: u64) -> u64 {
    let mut io = CountingIo::default();
    let Some(mut sender) = Sender::new([1; ID_LEN], &[RECEIVER]) else {
        return 0;
    };
    let sources: Vec<Ipv4Addr> = (1..=LINKS).map(|k| Ipv4Addr::new(10, k, 0, 2)).collect();
    sender.reload_sources(&sources, &mut io);
    sender.open_links(&mut io);
    let links: Vec<LinkHandle> = sender.links(0).map(|link| link.handle).collect();
    let mut reg2 = control(PacketType::Reg2, TYPE_LEN + ID_LEN);
    reg2[TYPE_LEN..TYPE_LEN + HALF_ID_LEN].fill(1);
    let Some(&first) = links.first() else {
        return 0;
    };
    sender.handle_link_datagram(0, first, &control(PacketType::RegNgp, TYPE_LEN), &mut io);
    sender.handle_link_datagram(0, first, &reg2, &mut io);
    for link in &links {
        sender.handle_link_datagram(0, *link, &control(PacketType::Reg3, TYPE_LEN), &mut io);
    }
    let mut data = [0u8; DATA_PACKET_BYTES];
    let mut srtla_ack = control(PacketType::SrtlaAck, 44);
    let mut srt_ack = control(PacketType::SrtAck, 44);
    let mut nak = control(PacketType::SrtNak, 24);
    for index in 0..packets {
        let number = u32::try_from(index & 0x7FFF_FFFF).unwrap_or(0);
        data[..4].copy_from_slice(&number.to_be_bytes());
        sender.handle_srt_datagram(0, CALLER, black_box(&data), &mut io);
        let on = links
            .get(usize::try_from(index % u64::from(LINKS)).unwrap_or(0))
            .copied()
            .unwrap_or(first);
        if number % PACKETS_PER_ACK == PACKETS_PER_ACK - 1 {
            for (slot, acked) in srtla_ack[4..]
                .chunks_exact_mut(4)
                .zip(number + 1 - PACKETS_PER_ACK..=number)
            {
                slot.copy_from_slice(&acked.to_be_bytes());
            }
            sender.handle_link_datagram(0, on, black_box(&srtla_ack), &mut io);
            srt_ack[16..20].copy_from_slice(&(number / 2).to_be_bytes());
            sender.handle_link_datagram(0, on, black_box(&srt_ack), &mut io);
        }
        if number % PACKETS_PER_NAK == PACKETS_PER_NAK / 2 {
            nak[16..20].copy_from_slice(&(0x8000_0000 | (number - 20)).to_be_bytes());
            nak[20..24].copy_from_slice(&(number - 18).to_be_bytes());
            sender.handle_link_datagram(0, on, black_box(&nak), &mut io);
        }
    }
    io.bytes
}
