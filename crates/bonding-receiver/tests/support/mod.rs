//! A recording [`ReceiverIo`] and packet builders for receiver tests.

#![allow(dead_code)]

use std::error::Error;
use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::srtla::{GroupId, HALF_ID_LEN, ID_LEN, PacketType};
use bonding_receiver::srtla::{GroupHandle, IoFailure, Receiver, ReceiverEvent, ReceiverIo};

pub type TestResult = Result<(), Box<dyn Error>>;

/// Where a datagram went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sent {
    ToLink(SocketAddrV4, Vec<u8>),
    ToSrt(GroupHandle, Vec<u8>),
}

/// An operation of [`ReceiverIo`] that can be made to fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    SendToLink,
    OpenSrt,
    SendToSrt,
    Randomness,
}

/// Records everything the receiver does; each operation can be made to fail.
#[derive(Debug, Default)]
pub struct FakeIo {
    pub sent: Vec<Sent>,
    pub opened: Vec<GroupHandle>,
    pub closed: Vec<GroupHandle>,
    pub events: Vec<ReceiverEvent>,
    /// Operations that fail.
    pub failing: Vec<Operation>,
    /// Bytes for the next random fills (each fill repeats one byte); then a counter.
    pub random_bytes: Vec<u8>,
    pub random_counter: u8,
}

impl FakeIo {
    /// A fake where `operations` fail.
    pub fn failing(operations: &[Operation]) -> Self {
        Self {
            failing: operations.to_vec(),
            ..Self::default()
        }
    }

    fn fails(&self, operation: Operation) -> Result<(), IoFailure> {
        if self.failing.contains(&operation) {
            Err(IoFailure)
        } else {
            Ok(())
        }
    }

    /// What was sent since the last call, and forgets it.
    pub fn take_sent(&mut self) -> Vec<Sent> {
        std::mem::take(&mut self.sent)
    }

    /// Datagrams sent to one link since the last `take_sent`.
    pub fn to_link(&self, link: SocketAddrV4) -> Vec<Vec<u8>> {
        self.sent
            .iter()
            .filter_map(|sent| match sent {
                Sent::ToLink(to, bytes) if *to == link => Some(bytes.clone()),
                _ => None,
            })
            .collect()
    }

    /// Datagrams forwarded to SRT since the last `take_sent`.
    pub fn to_srt(&self) -> Vec<Vec<u8>> {
        self.sent
            .iter()
            .filter_map(|sent| match sent {
                Sent::ToSrt(_, bytes) => Some(bytes.clone()),
                Sent::ToLink(..) => None,
            })
            .collect()
    }
}

impl ReceiverIo for FakeIo {
    fn send_to_link(&mut self, link: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.fails(Operation::SendToLink)?;
        self.sent.push(Sent::ToLink(link, datagram.to_vec()));
        Ok(())
    }

    fn open_srt(&mut self, group: GroupHandle) -> Result<(), IoFailure> {
        self.fails(Operation::OpenSrt)?;
        self.opened.push(group);
        Ok(())
    }

    fn send_to_srt(&mut self, group: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure> {
        self.fails(Operation::SendToSrt)?;
        self.sent.push(Sent::ToSrt(group, datagram.to_vec()));
        Ok(())
    }

    fn close_srt(&mut self, group: GroupHandle) {
        self.closed.push(group);
    }

    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure> {
        self.fails(Operation::Randomness)?;
        let byte = if self.random_bytes.is_empty() {
            self.random_counter = self.random_counter.wrapping_add(1);
            self.random_counter
        } else {
            self.random_bytes.remove(0)
        };
        bytes.fill(byte);
        Ok(())
    }

    fn report(&mut self, event: ReceiverEvent) {
        self.events.push(event);
    }
}

/// Link `k` of box `box_number`: address 10.k.0.2 (box 1) or 10.k.1.2 (box 2…), port 40000 + k.
pub fn link_of(box_number: u8, k: u8) -> SocketAddrV4 {
    SocketAddrV4::new(
        Ipv4Addr::new(10, k, box_number - 1, 2),
        40_000 + u16::from(k),
    )
}

/// Link `k` of the first box.
pub fn link(k: u8) -> SocketAddrV4 {
    link_of(1, k)
}

/// A 2-byte packet of that type (REG3, `REG_ERR`, keepalive…).
pub fn answer(kind: PacketType) -> Vec<u8> {
    kind.raw().to_be_bytes().to_vec()
}

/// REG1 whose sender half is all `half`.
pub fn reg1(half: u8) -> Vec<u8> {
    let mut packet = answer(PacketType::Reg1);
    packet.extend([half; HALF_ID_LEN]);
    packet.extend([0; HALF_ID_LEN]);
    packet
}

/// REG2 carrying `id`.
pub fn reg2(id: &GroupId) -> Vec<u8> {
    let mut packet = answer(PacketType::Reg2);
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

/// The full group id in a REG2 answer, if `packet` is one.
pub fn id_in(packet: &[u8]) -> Option<GroupId> {
    let (kind, id) = packet.split_at_checked(2)?;
    (kind == answer(PacketType::Reg2).as_slice() && id.len() == ID_LEN)
        .then(|| id.try_into().ok())
        .flatten()
}

/// Registers one box on `links` (REG1 on the first, REG2 on each) at `now_s`; returns its
/// group and full id, and clears what was sent.
pub fn register(
    receiver: &mut Receiver,
    io: &mut FakeIo,
    now_s: u64,
    half: u8,
    links: &[SocketAddrV4],
) -> Result<(GroupHandle, GroupId), Box<dyn Error>> {
    let first = *links.first().ok_or("no links")?;
    receiver.handle_link_datagram(now_s, first, &reg1(half), io);
    let id = io
        .to_link(first)
        .last()
        .and_then(|answer| id_in(answer))
        .ok_or("no REG2 answer")?;
    for link in links {
        receiver.handle_link_datagram(now_s, *link, &reg2(&id), io);
    }
    let group = receiver.group_of(first).ok_or("not registered")?;
    io.take_sent();
    Ok((group, id))
}
