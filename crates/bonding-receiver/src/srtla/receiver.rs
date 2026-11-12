//! The receiver's decisions, one datagram at a time.

use std::net::SocketAddrV4;

use bonding_protocol::srtla::{
    GroupId, HALF_ID_LEN, Packet, PacketType, REG2_LEN, SEQUENCE_NUMBER_LEN, SRT_MIN_LEN,
    SRTLA_ACK_HEADER_LEN, SequenceNumber, SrtlaAck, data_sequence_number, raw_type,
};

use super::address_owner::AddressOwner;
use super::group::Group;
use super::group_handle::GroupHandle;
use super::limits::{
    ACK_BATCH_LEN, CLEANUP_PERIOD_S, EMPTY_GROUP_TIMEOUT_S, LINK_TIMEOUT_S, MAX_GROUPS,
    MAX_LINKS_PER_GROUP,
};
use super::link::Link;
use super::receiver_event::{GroupRemoval, ReceiverEvent, Refusal};
use super::receiver_io::ReceiverIo;

const SRTLA_ACK_LEN: usize = SRTLA_ACK_HEADER_LEN + ACK_BATCH_LEN * SEQUENCE_NUMBER_LEN;
const KEEPALIVE: u16 = PacketType::Keepalive.raw();
const SRT_ACK: u16 = PacketType::SrtAck.raw();

/// The SRTLA receiver, as upstream `srtla_rec` behaves.
///
/// Feed it every datagram with the time in whole seconds of a monotonic clock (upstream's
/// resolution), and call [`housekeeping`](Self::housekeeping) at least once a second. It only
/// allocates when a group is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receiver {
    /// Oldest first; upstream looks them up newest first.
    groups: Vec<Group>,
    next_handle: u64,
    last_cleanup_s: Option<u64>,
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

impl Receiver {
    /// A receiver with no groups.
    #[must_use]
    pub fn new() -> Self {
        Self {
            groups: Vec::with_capacity(MAX_GROUPS),
            next_handle: 0,
            last_cleanup_s: None,
        }
    }

    /// A datagram from a sender's link, on the listening socket.
    pub fn handle_link_datagram(
        &mut self,
        now_s: u64,
        from: SocketAddrV4,
        datagram: &[u8],
        io: &mut impl ReceiverIo,
    ) {
        match Packet::decode(datagram) {
            Ok(Packet::Reg1(id)) => return self.register_group(now_s, from, id, io),
            Ok(Packet::Reg2(id)) => return self.register_link(now_s, from, id, io),
            _ => {}
        }
        let AddressOwner::Link { group, link } = self.owner_of(from) else {
            return;
        };
        let Some(owner) = self.groups.get_mut(group) else {
            return;
        };
        let Some(sender_link) = owner.links.get_mut(link) else {
            return;
        };
        sender_link.last_received_s = now_s;
        if raw_type(datagram) == Some(KEEPALIVE) {
            send_to_link(io, from, datagram);
            return;
        }
        if datagram.len() < SRT_MIN_LEN {
            return;
        }
        owner.last_address = from;
        let full_batch = data_sequence_number(datagram)
            .and_then(SequenceNumber::new)
            .and_then(|number| sender_link.ack_batch.push(number));
        if let Some(batch) = full_batch {
            send_srtla_ack(io, from, &batch);
        }
        let handle = owner.handle;
        if !owner.srt_open {
            if io.open_srt(handle).is_err() {
                self.remove_group(group, GroupRemoval::SrtFailed, io);
                return;
            }
            owner.srt_open = true;
        }
        if io.send_to_srt(handle, datagram).is_err() {
            self.remove_group(group, GroupRemoval::SrtFailed, io);
        }
    }

    /// A datagram from the SRT listener, on a group's socket.
    pub fn handle_srt_datagram(
        &mut self,
        group: GroupHandle,
        datagram: &[u8],
        io: &mut impl ReceiverIo,
    ) {
        let Some(index) = self.index_of(group) else {
            return;
        };
        if datagram.len() < SRT_MIN_LEN {
            self.remove_group(index, GroupRemoval::SrtFailed, io);
            return;
        }
        let Some(owner) = self.groups.get(index) else {
            return;
        };
        if raw_type(datagram) == Some(SRT_ACK) {
            for link in owner.links.iter().rev() {
                send_to_link(io, link.address, datagram);
            }
        } else {
            send_to_link(io, owner.last_address, datagram);
        }
    }

    /// Reading a group's SRT socket failed (e.g. the listener restarted): the group is removed,
    /// as upstream does.
    pub fn handle_srt_failure(&mut self, group: GroupHandle, io: &mut impl ReceiverIo) {
        if let Some(index) = self.index_of(group) {
            self.remove_group(index, GroupRemoval::SrtFailed, io);
        }
    }

    /// Forgets silent links and empty groups; does nothing more often than every
    /// [`CLEANUP_PERIOD_S`].
    pub fn housekeeping(&mut self, now_s: u64, io: &mut impl ReceiverIo) {
        if self
            .last_cleanup_s
            .is_some_and(|last_s| last_s + CLEANUP_PERIOD_S > now_s)
        {
            return;
        }
        self.last_cleanup_s = Some(now_s);
        for index in (0..self.groups.len()).rev() {
            let Some(group) = self.groups.get_mut(index) else {
                continue;
            };
            let handle = group.handle;
            group.links.retain(|link| {
                let alive = link.last_received_s + LINK_TIMEOUT_S >= now_s;
                if !alive {
                    io.report(ReceiverEvent::LinkTimedOut {
                        group: handle,
                        link: link.address,
                    });
                }
                alive
            });
            if group.links.is_empty() && group.created_s + EMPTY_GROUP_TIMEOUT_S < now_s {
                self.remove_group(index, GroupRemoval::NoLinks, io);
            }
        }
    }

    /// How many groups exist.
    #[must_use]
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// The group a link is registered in.
    #[must_use]
    pub fn group_of(&self, link: SocketAddrV4) -> Option<GroupHandle> {
        match self.owner_of(link) {
            AddressOwner::Link { group, .. } => self.groups.get(group).map(|group| group.handle),
            _ => None,
        }
    }

    /// The links of a group, newest first.
    pub fn links(&self, group: GroupHandle) -> impl Iterator<Item = SocketAddrV4> + '_ {
        self.index_of(group)
            .and_then(|index| self.groups.get(index))
            .into_iter()
            .flat_map(|group| group.links.iter().rev().map(|link| link.address))
    }

    fn register_group(
        &mut self,
        now_s: u64,
        from: SocketAddrV4,
        proposal: &GroupId,
        io: &mut impl ReceiverIo,
    ) {
        if self.groups.len() >= MAX_GROUPS {
            return refuse_group(io, from, Refusal::TooManyGroups);
        }
        if self.owner_of(from) != AddressOwner::Nobody {
            return refuse_group(io, from, Refusal::AddressInUse);
        }
        let mut id = *proposal;
        loop {
            let Some(receiver_half) = id.get_mut(HALF_ID_LEN..) else {
                return refuse_group(io, from, Refusal::NoRandomness);
            };
            if io.fill_random(receiver_half).is_err() {
                return refuse_group(io, from, Refusal::NoRandomness);
            }
            if !self.groups.iter().any(|group| group.has_id(&id)) {
                break;
            }
        }
        let mut answer = [0u8; REG2_LEN];
        let sent = Packet::Reg2(&id)
            .encode(&mut answer)
            .ok()
            .and_then(|len| answer.get(..len))
            .is_some_and(|reg2| io.send_to_link(from, reg2).is_ok());
        if !sent {
            return refuse_group(io, from, Refusal::AnswerNotSent);
        }
        let handle = GroupHandle::new(self.next_handle);
        self.next_handle += 1;
        self.groups.push(Group::new(handle, id, now_s, from));
        io.report(ReceiverEvent::GroupRegistered {
            group: handle,
            from,
        });
    }

    fn register_link(
        &mut self,
        now_s: u64,
        from: SocketAddrV4,
        id: &GroupId,
        io: &mut impl ReceiverIo,
    ) {
        let Some(index) = self.groups.iter().rposition(|group| group.has_id(id)) else {
            send_answer(io, from, PacketType::RegNgp);
            io.report(ReceiverEvent::LinkRefused {
                link: from,
                reason: Refusal::UnknownGroup,
            });
            return;
        };
        let known_link = match self.owner_of(from) {
            AddressOwner::Link { group, link } if group == index => Some(link),
            AddressOwner::Group(group) if group == index => None,
            AddressOwner::Nobody => None,
            _ => return refuse_link(io, from, Refusal::AddressInUse),
        };
        let Some(group) = self.groups.get_mut(index) else {
            return;
        };
        let link = match known_link {
            Some(link) => link,
            None if group.links.len() >= MAX_LINKS_PER_GROUP => {
                return refuse_link(io, from, Refusal::GroupFull);
            }
            None => {
                group.links.push(Link::new(from, now_s));
                group.links.len() - 1
            }
        };
        if !send_answer(io, from, PacketType::Reg3) {
            group.links.remove(link);
            return refuse_link(io, from, Refusal::AnswerNotSent);
        }
        group.last_address = from;
        io.report(ReceiverEvent::LinkRegistered {
            group: group.handle,
            link: from,
            again: known_link.is_some(),
        });
    }

    fn owner_of(&self, address: SocketAddrV4) -> AddressOwner {
        for (group_index, group) in self.groups.iter().enumerate().rev() {
            if let Some(link) = group.links.iter().rposition(|link| link.address == address) {
                return AddressOwner::Link {
                    group: group_index,
                    link,
                };
            }
            if group.last_address == address {
                return AddressOwner::Group(group_index);
            }
        }
        AddressOwner::Nobody
    }

    fn index_of(&self, handle: GroupHandle) -> Option<usize> {
        self.groups.iter().position(|group| group.handle == handle)
    }

    fn remove_group(&mut self, index: usize, reason: GroupRemoval, io: &mut impl ReceiverIo) {
        if index >= self.groups.len() {
            return;
        }
        let group = self.groups.remove(index);
        if group.srt_open {
            io.close_srt(group.handle);
        }
        io.report(ReceiverEvent::GroupRemoved {
            group: group.handle,
            reason,
        });
    }
}

fn send_to_link(io: &mut impl ReceiverIo, link: SocketAddrV4, datagram: &[u8]) -> bool {
    let sent = io.send_to_link(link, datagram).is_ok();
    if !sent {
        io.report(ReceiverEvent::SendToLinkFailed { link });
    }
    sent
}

fn send_answer(io: &mut impl ReceiverIo, link: SocketAddrV4, kind: PacketType) -> bool {
    send_to_link(io, link, &kind.raw().to_be_bytes())
}

fn send_srtla_ack(io: &mut impl ReceiverIo, link: SocketAddrV4, batch: &[SequenceNumber]) {
    let mut ack = [0u8; SRTLA_ACK_LEN];
    if let Some(datagram) = SrtlaAck::encode(batch, &mut ack)
        .ok()
        .and_then(|len| ack.get(..len))
    {
        send_to_link(io, link, datagram);
    }
}

fn refuse_group(io: &mut impl ReceiverIo, from: SocketAddrV4, reason: Refusal) {
    send_answer(io, from, PacketType::RegErr);
    io.report(ReceiverEvent::GroupRefused { from, reason });
}

fn refuse_link(io: &mut impl ReceiverIo, link: SocketAddrV4, reason: Refusal) {
    send_answer(io, link, PacketType::RegErr);
    io.report(ReceiverEvent::LinkRefused { link, reason });
}
