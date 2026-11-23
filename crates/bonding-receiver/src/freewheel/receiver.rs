//! The freewheel receiver's decisions, one datagram at a time.

use std::net::SocketAddrV4;

use bonding_protocol::freewheel::{
    Ack, CloseReason, Data, Header, MAX_DATAGRAM_LEN, Packet, TrafficClass,
};

use super::limits::{
    ACK_EVERY_PACKETS, ACK_MAX_DELAY_US, LINK_TIMEOUT_US, MAX_LINKS_PER_SESSION, MAX_SESSIONS,
    SESSION_TIMEOUT_US,
};
use super::link::Link;
use super::session::Session;
use crate::srtla::{GroupHandle, GroupRemoval, ReceiverEvent, ReceiverIo, Refusal};

/// The freewheel receiver: sessions of links, each forwarded to the SRT listener.
///
/// Feed it every datagram with the time in microseconds of a monotonic clock, and call
/// [`housekeeping`](Self::housekeeping) at least every [`ACK_MAX_DELAY_US`]. It only
/// allocates when a session is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receiver {
    sessions: Vec<Session>,
    next_handle: u64,
}

impl Default for Receiver {
    fn default() -> Self {
        Self::new()
    }
}

impl Receiver {
    /// A receiver with no sessions.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Vec::with_capacity(MAX_SESSIONS),
            next_handle: 0,
        }
    }

    /// A datagram from a sender's link, on the listening socket.
    pub fn handle_link_datagram(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        datagram: &[u8],
        io: &mut impl ReceiverIo,
    ) {
        match Packet::decode(datagram) {
            Ok(Packet::Join(header)) => self.join(now_us, from, header, io),
            Ok(Packet::Data(data)) => self.receive_data(now_us, from, &data, datagram.len(), io),
            Ok(Packet::Keepalive { header, sent_at_us }) => {
                let Some((_, link)) = self.heard(now_us, from, header, io) else {
                    return;
                };
                link.latest_sent_at_us = sent_at_us;
                link.latest_arrival_us = now_us;
                let link = *link;
                send_ack(io, header.session_id, &link, now_us);
                if let Some(link) = self.find_link(header) {
                    link.unacknowledged_packets = 0;
                }
            }
            Ok(Packet::Close { header, .. }) => self.close_link(now_us, header),
            _ => {}
        }
    }

    /// A datagram from the SRT listener, on a session's socket: back to the sender, through
    /// the link that delivered last.
    pub fn handle_srt_datagram(
        &mut self,
        now_us: u64,
        group: GroupHandle,
        datagram: &[u8],
        io: &mut impl ReceiverIo,
    ) {
        let Some(session) = self.sessions.iter_mut().find(|s| s.handle == group) else {
            return;
        };
        let Some(link) = session
            .return_link
            .and_then(|id| session.links.iter().find(|link| link.id == id))
        else {
            return;
        };
        let packet = Packet::Data(Data {
            header: Header {
                session_id: session.id,
                link_id: link.id,
            },
            class: TrafficClass::Live,
            sequence: session.next_sequence,
            sent_at_us: wrapped_us(now_us),
            payload: datagram,
        });
        session.next_sequence = session.next_sequence.wrapping_add(1);
        let address = link.address;
        send_packet(io, address, &packet);
    }

    /// Reading a session's SRT socket failed: the session is removed.
    pub fn handle_srt_failure(&mut self, group: GroupHandle, io: &mut impl ReceiverIo) {
        if let Some(index) = self.sessions.iter().position(|s| s.handle == group) {
            self.remove_session(index, GroupRemoval::SrtFailed, io);
        }
    }

    /// Sends ACKs that waited [`ACK_MAX_DELAY_US`], forgets silent links and empty sessions.
    pub fn housekeeping(&mut self, now_us: u64, io: &mut impl ReceiverIo) {
        for index in (0..self.sessions.len()).rev() {
            let Some(session) = self.sessions.get_mut(index) else {
                continue;
            };
            let (session_id, handle) = (session.id, session.handle);
            for link in &mut session.links {
                if link.unacknowledged_packets > 0
                    && now_us.saturating_sub(link.latest_arrival_us) >= ACK_MAX_DELAY_US
                {
                    send_ack(io, session_id, link, now_us);
                    link.unacknowledged_packets = 0;
                }
            }
            let had_links = !session.links.is_empty();
            session.links.retain(|link| {
                let alive = now_us.saturating_sub(link.last_received_us) < LINK_TIMEOUT_US;
                if !alive {
                    io.report(ReceiverEvent::LinkTimedOut {
                        group: handle,
                        link: link.address,
                    });
                }
                alive
            });
            if had_links && session.links.is_empty() {
                session.empty_since_us = now_us;
            }
            if session.links.is_empty()
                && now_us.saturating_sub(session.empty_since_us) > SESSION_TIMEOUT_US
            {
                self.remove_session(index, GroupRemoval::NoLinks, io);
            }
        }
    }

    /// How many sessions exist.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// The links of a session, by address.
    pub fn links(&self, group: GroupHandle) -> impl Iterator<Item = SocketAddrV4> + '_ {
        self.sessions
            .iter()
            .filter(move |session| session.handle == group)
            .flat_map(|session| session.links.iter().map(|link| link.address))
    }

    fn join(&mut self, now_us: u64, from: SocketAddrV4, header: Header, io: &mut impl ReceiverIo) {
        let index = match self.sessions.iter().position(|s| s.id == header.session_id) {
            Some(index) => index,
            None if self.sessions.len() >= MAX_SESSIONS => {
                return refuse(io, from, header, Refusal::TooManyGroups);
            }
            None => {
                let handle = GroupHandle::new(self.next_handle);
                self.next_handle += 1;
                self.sessions
                    .push(Session::new(header.session_id, handle, now_us));
                io.report(ReceiverEvent::GroupRegistered {
                    group: handle,
                    from,
                });
                self.sessions.len() - 1
            }
        };
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        let again = if let Some(link) = session.link_mut(header.link_id) {
            link.address = from;
            link.last_received_us = now_us;
            true
        } else if session.links.len() >= MAX_LINKS_PER_SESSION {
            return refuse(io, from, header, Refusal::GroupFull);
        } else {
            session.links.push(Link::new(header.link_id, from, now_us));
            false
        };
        let handle = session.handle;
        send_packet(io, from, &Packet::Accept(header));
        io.report(ReceiverEvent::LinkRegistered {
            group: handle,
            link: from,
            again,
        });
    }

    fn receive_data(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        data: &Data<'_>,
        datagram_len: usize,
        io: &mut impl ReceiverIo,
    ) {
        let Some((index, link)) = self.heard(now_us, from, data.header, io) else {
            return;
        };
        link.latest_sequence = data.sequence;
        link.latest_sent_at_us = data.sent_at_us;
        link.latest_arrival_us = now_us;
        link.received_bytes = link
            .received_bytes
            .wrapping_add(u32::try_from(datagram_len).unwrap_or(u32::MAX));
        link.unacknowledged_packets += 1;
        if link.unacknowledged_packets >= ACK_EVERY_PACKETS {
            link.unacknowledged_packets = 0;
            let link = *link;
            send_ack(io, data.header.session_id, &link, now_us);
        }
        // Background data is counted and acknowledged, so the sender measures the link, but
        // goes nowhere yet: the sender's capacity probing pads with it (the recording upload,
        // M1b step 6, will use the same class).
        if data.class == TrafficClass::Background {
            return;
        }
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        session.return_link = Some(data.header.link_id);
        let handle = session.handle;
        if !session.srt_open {
            if io.open_srt(handle).is_err() {
                self.remove_session(index, GroupRemoval::SrtFailed, io);
                return;
            }
            session.srt_open = true;
        }
        if io.send_to_srt(handle, data.payload).is_err() {
            self.remove_session(index, GroupRemoval::SrtFailed, io);
        }
    }

    /// A packet from a known link: its address and clock are updated. From an unknown one,
    /// the sender is told to join again.
    fn heard(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        header: Header,
        io: &mut impl ReceiverIo,
    ) -> Option<(usize, &mut Link)> {
        let index = self
            .sessions
            .iter()
            .position(|session| session.id == header.session_id);
        let Some(link) = index
            .and_then(|index| self.sessions.get_mut(index))
            .and_then(|session| session.link_mut(header.link_id))
        else {
            send_packet(
                io,
                from,
                &Packet::Close {
                    header,
                    reason: CloseReason::Refused,
                },
            );
            return None;
        };
        link.address = from;
        link.last_received_us = now_us;
        index.map(|index| (index, link))
    }

    fn find_link(&mut self, header: Header) -> Option<&mut Link> {
        self.sessions
            .iter_mut()
            .find(|session| session.id == header.session_id)
            .and_then(|session| session.link_mut(header.link_id))
    }

    fn close_link(&mut self, now_us: u64, header: Header) {
        if let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.id == header.session_id)
        {
            session.links.retain(|link| link.id != header.link_id);
            if session.links.is_empty() {
                session.empty_since_us = now_us;
            }
        }
    }

    fn remove_session(&mut self, index: usize, reason: GroupRemoval, io: &mut impl ReceiverIo) {
        if index >= self.sessions.len() {
            return;
        }
        let session = self.sessions.remove(index);
        if session.srt_open {
            io.close_srt(session.handle);
        }
        io.report(ReceiverEvent::GroupRemoved {
            group: session.handle,
            reason,
        });
    }
}

/// The low 32 bits of a microsecond clock, as the wire carries it.
const fn wrapped_us(now_us: u64) -> u32 {
    (now_us & 0xFFFF_FFFF) as u32
}

fn send_packet(io: &mut impl ReceiverIo, to: SocketAddrV4, packet: &Packet<'_>) {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let Some(datagram) = packet.encode(&mut out).ok().and_then(|len| out.get(..len)) else {
        return;
    };
    if io.send_to_link(to, datagram).is_err() {
        io.report(ReceiverEvent::SendToLinkFailed { link: to });
    }
}

fn send_ack(io: &mut impl ReceiverIo, session_id: u64, link: &Link, now_us: u64) {
    let header = Header {
        session_id,
        link_id: link.id,
    };
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let Some(datagram) = Ack::encode(header, link.timing(now_us), &[], &mut out)
        .ok()
        .and_then(|len| out.get(..len))
    else {
        return;
    };
    if io.send_to_link(link.address, datagram).is_err() {
        io.report(ReceiverEvent::SendToLinkFailed { link: link.address });
    }
}

fn refuse(io: &mut impl ReceiverIo, from: SocketAddrV4, header: Header, reason: Refusal) {
    send_packet(
        io,
        from,
        &Packet::Close {
            header,
            reason: CloseReason::Refused,
        },
    );
    io.report(ReceiverEvent::LinkRefused { link: from, reason });
}
