//! A recording [`SenderIo`] and packet builders for sender tests.

#![allow(dead_code)]

use std::error::Error;
use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::srtla::{GroupId, HALF_ID_LEN, ID_LEN, PacketType};
use bonding_sender::srtla::{IoFailure, LinkHandle, Sender, SenderEvent, SenderIo};

pub type TestResult = Result<(), Box<dyn Error>>;

/// The receiver every test sender talks to.
pub const RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), 5_000);
/// The receiver's second address, tried when the first never answers.
pub const FALLBACK_RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 2), 5_000);
/// The SRT caller on the box.
pub const CALLER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7_001);
/// The sender's half of every test group id.
pub const SENDER_HALF: u8 = 0x11;
/// The receiver's half of every test group id.
pub const RECEIVER_HALF: u8 = 0x22;

/// Where a datagram went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sent {
    OnLink(LinkHandle, SocketAddrV4, Vec<u8>),
    ToSrt(SocketAddrV4, Vec<u8>),
}

/// Records everything the sender does; sockets of `unbindable` sources and sends on
/// `failing_links` fail.
#[derive(Debug, Default)]
pub struct FakeIo {
    pub sent: Vec<Sent>,
    pub opened: Vec<LinkHandle>,
    pub closed: Vec<LinkHandle>,
    pub events: Vec<SenderEvent>,
    pub unbindable: Vec<Ipv4Addr>,
    pub failing_links: Vec<LinkHandle>,
}

impl FakeIo {
    /// What was sent since the last call, and forgets it.
    pub fn take_sent(&mut self) -> Vec<Sent> {
        std::mem::take(&mut self.sent)
    }

    /// Datagrams sent on one link since the last `take_sent`.
    pub fn on_link(&self, link: LinkHandle) -> Vec<Vec<u8>> {
        self.sent
            .iter()
            .filter_map(|sent| match sent {
                Sent::OnLink(on, _, bytes) if *on == link => Some(bytes.clone()),
                _ => None,
            })
            .collect()
    }

    /// Datagrams sent back to the SRT caller since the last `take_sent`.
    pub fn to_srt(&self) -> Vec<Vec<u8>> {
        self.sent
            .iter()
            .filter_map(|sent| match sent {
                Sent::ToSrt(_, bytes) => Some(bytes.clone()),
                Sent::OnLink(..) => None,
            })
            .collect()
    }

    /// The link each data packet went on, in order, since the last `take_sent`.
    pub fn data_links(&self) -> Vec<LinkHandle> {
        self.sent
            .iter()
            .filter_map(|sent| match sent {
                Sent::OnLink(link, _, bytes) if bytes.first().is_some_and(|b| b & 0x80 == 0) => {
                    Some(*link)
                }
                _ => None,
            })
            .collect()
    }
}

impl SenderIo for FakeIo {
    fn open_link(&mut self, link: LinkHandle, source: Ipv4Addr) -> Result<(), IoFailure> {
        if self.unbindable.contains(&source) {
            return Err(IoFailure);
        }
        self.opened.push(link);
        Ok(())
    }

    fn close_link(&mut self, link: LinkHandle) {
        self.closed.push(link);
    }

    fn send_on_link(
        &mut self,
        link: LinkHandle,
        receiver: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), IoFailure> {
        if self.failing_links.contains(&link) {
            return Err(IoFailure);
        }
        self.sent
            .push(Sent::OnLink(link, receiver, datagram.to_vec()));
        Ok(())
    }

    fn send_to_srt(&mut self, caller: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.sent.push(Sent::ToSrt(caller, datagram.to_vec()));
        Ok(())
    }

    fn report(&mut self, event: SenderEvent) {
        self.events.push(event);
    }
}

/// Source address of modem `k`: 10.k.0.2, as in the lab.
pub fn source(k: u8) -> Ipv4Addr {
    Ipv4Addr::new(10, k, 0, 2)
}

/// The id a new test sender proposes: its half, then zeros.
pub fn proposed_id() -> GroupId {
    let mut id = [0; ID_LEN];
    id[..HALF_ID_LEN].fill(SENDER_HALF);
    id
}

/// The id after registration: our half, then the receiver's.
pub fn full_id() -> GroupId {
    let mut id = proposed_id();
    id[HALF_ID_LEN..].fill(RECEIVER_HALF);
    id
}

/// A 2-byte packet of that type (REG3, `REG_NGP`, keepalive…).
pub fn answer(kind: PacketType) -> Vec<u8> {
    kind.raw().to_be_bytes().to_vec()
}

/// REG1 or REG2 carrying `id`.
pub fn registration(kind: PacketType, id: &GroupId) -> Vec<u8> {
    let mut packet = answer(kind);
    packet.extend(id);
    packet
}

/// An SRT data packet: sequence number, then 28 payload bytes.
pub fn data(sequence_number: u32) -> Vec<u8> {
    let mut packet = sequence_number.to_be_bytes().to_vec();
    packet.extend([0xAB; 28]);
    packet
}

/// An SRT control packet of that type, 44 bytes.
pub fn srt_control(kind: PacketType) -> Vec<u8> {
    let mut packet = answer(kind);
    packet.extend([0; 42]);
    packet
}

/// An SRT ACK of everything before `number`.
pub fn srt_ack(number: u32) -> Vec<u8> {
    let mut packet = srt_control(PacketType::SrtAck);
    packet[16..20].copy_from_slice(&number.to_be_bytes());
    packet
}

/// An SRT NAK with that raw loss list.
pub fn srt_nak(loss_list: &[u32]) -> Vec<u8> {
    let mut packet = answer(PacketType::SrtNak);
    packet.extend([0; 14]);
    for word in loss_list {
        packet.extend(word.to_be_bytes());
    }
    packet
}

/// An SRTLA ACK of `numbers`.
pub fn srtla_ack(numbers: &[u32]) -> Vec<u8> {
    let mut packet = answer(PacketType::SrtlaAck);
    packet.extend([0, 0]);
    for number in numbers {
        packet.extend(number.to_be_bytes());
    }
    packet
}

/// A sender on modems `ks` (file order), sockets open, nothing sent yet.
pub fn sender_on(ks: &[u8], io: &mut FakeIo) -> Result<Sender, Box<dyn Error>> {
    let mut sender =
        Sender::new(proposed_id(), &[RECEIVER, FALLBACK_RECEIVER]).ok_or("no receiver")?;
    let sources: Vec<Ipv4Addr> = ks.iter().map(|k| source(*k)).collect();
    sender.reload_sources(&sources, io);
    sender.open_links(io);
    Ok(sender)
}

/// The handle of modem `k`'s link.
pub fn link(sender: &Sender, k: u8) -> Result<LinkHandle, Box<dyn Error>> {
    Ok(sender.link_of(source(k)).ok_or("no such link")?)
}

/// Registers every link at `now_ms`, as upstream's exchange goes: housekeeping sends REG2,
/// `REG_NGP` on the first link, REG1, REG2 back, REG2 everywhere, REG3 for each. Clears what
/// was sent.
pub fn connect(sender: &mut Sender, io: &mut FakeIo, now_ms: u64) -> TestResult {
    sender.housekeeping(now_ms, io);
    let handles: Vec<LinkHandle> = sender.links(now_ms).map(|link| link.handle).collect();
    let first = *handles.first().ok_or("no links")?;
    sender.handle_link_datagram(now_ms, first, &answer(PacketType::RegNgp), io);
    sender.handle_link_datagram(
        now_ms,
        first,
        &registration(PacketType::Reg2, &full_id()),
        io,
    );
    for handle in handles {
        sender.handle_link_datagram(now_ms, handle, &answer(PacketType::Reg3), io);
    }
    io.take_sent();
    io.events.clear();
    Ok(())
}

/// The two loss-list words of a range of lost packets, `first` to `last` inclusive.
pub fn loss_range(first: u32, last: u32) -> [u32; 2] {
    [0x8000_0000 | first, last]
}
