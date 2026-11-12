//! The per-packet path allocates nothing: the count does not grow with traffic.

use std::net::{Ipv4Addr, SocketAddrV4};

use alloc_counter::{CountingAllocator, count_allocations};
use bonding_protocol::srtla::{HALF_ID_LEN, ID_LEN, PacketType};
use bonding_sender::srtla::{IoFailure, LinkHandle, Sender, SenderIo};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Counts bytes; never allocates.
#[derive(Default)]
struct CountingIo {
    bytes_sent: usize,
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
        self.bytes_sent += datagram.len();
        Ok(())
    }
    fn send_to_srt(&mut self, _: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.bytes_sent += datagram.len();
        Ok(())
    }
}

const RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), 5_000);
const CALLER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7_001);

fn stream(packets: u32) -> usize {
    let mut io = CountingIo::default();
    let Some(mut sender) = Sender::new([1; ID_LEN], &[RECEIVER]) else {
        return 0;
    };
    let sources: Vec<Ipv4Addr> = (1..=4).map(|k| Ipv4Addr::new(10, k, 0, 2)).collect();
    sender.reload_sources(&sources, &mut io);
    sender.open_links(&mut io);
    let handles: Vec<LinkHandle> = sender.links(0).map(|link| link.handle).collect();
    let mut reg2 = [1u8; 2 + ID_LEN];
    reg2[..2].copy_from_slice(&PacketType::Reg2.raw().to_be_bytes());
    reg2[2 + HALF_ID_LEN..].fill(2);
    if let Some(first) = handles.first() {
        sender.handle_link_datagram(0, *first, &PacketType::RegNgp.raw().to_be_bytes(), &mut io);
        sender.handle_link_datagram(0, *first, &reg2, &mut io);
    }
    for handle in &handles {
        sender.handle_link_datagram(0, *handle, &PacketType::Reg3.raw().to_be_bytes(), &mut io);
    }
    let mut packet = [0u8; 1_316];
    let mut srt_ack = [0u8; 44];
    srt_ack[..2].copy_from_slice(&PacketType::SrtAck.raw().to_be_bytes());
    let mut srtla_ack = [0u8; 44];
    srtla_ack[..2].copy_from_slice(&PacketType::SrtlaAck.raw().to_be_bytes());
    let mut nak = [0u8; 24];
    nak[..2].copy_from_slice(&PacketType::SrtNak.raw().to_be_bytes());
    for number in 0..packets {
        let now_ms = u64::from(number);
        packet[..4].copy_from_slice(&number.to_be_bytes());
        sender.handle_srt_datagram(now_ms, CALLER, &packet, &mut io);
        let Some(on) = handles.get(usize::try_from(number % 4).unwrap_or(0)) else {
            continue;
        };
        if number % 10 == 9 {
            for (slot, acked) in srtla_ack[4..].chunks_exact_mut(4).zip(number - 9..=number) {
                slot.copy_from_slice(&acked.to_be_bytes());
            }
            sender.handle_link_datagram(now_ms, *on, &srtla_ack, &mut io);
            srt_ack[16..20].copy_from_slice(&(number - 5).to_be_bytes());
            sender.handle_link_datagram(now_ms, *on, &srt_ack, &mut io);
        }
        if number % 100 == 50 {
            nak[16..20].copy_from_slice(&(0x8000_0000 | (number - 20)).to_be_bytes());
            nak[20..24].copy_from_slice(&(number - 18).to_be_bytes());
            sender.handle_link_datagram(now_ms, *on, &nak, &mut io);
        }
        sender.housekeeping(now_ms, &mut io);
    }
    io.bytes_sent
}

#[test]
fn sending_allocates_nothing_per_packet() {
    let (_, small) = count_allocations(|| stream(1_000));
    let (_, large) = count_allocations(|| stream(100_000));
    assert_eq!(
        small, large,
        "{small} allocations for 1k packets, {large} for 100k"
    );
}
