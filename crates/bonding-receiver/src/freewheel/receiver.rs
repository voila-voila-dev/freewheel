//! The freewheel receiver's decisions, one datagram at a time.

use std::net::SocketAddrV4;

use bonding_protocol::freewheel::{
    Ack, BackgroundPayload, CloseReason, Data, Envelope, HELLO_MESSAGE_LEN, HandshakeMessage,
    Header, MAX_DATAGRAM_LEN, Packet, Sealed, SequenceRange, TrafficClass, WELCOME_MESSAGE_LEN,
};
use freewheel_noise::{HandshakeError, OpenError, ServerKeys, read_hello};

use super::limits::{
    ACK_EVERY_PACKETS, ACK_MAX_DELAY_US, LINK_TIMEOUT_US, MAX_LINKS_PER_SESSION, MAX_SESSIONS,
    SESSION_TIMEOUT_US,
};
use super::link::Link;
use super::recording_settings::{MAX_ACKNOWLEDGED_RANGES, RecordingSettings};
use super::rejected_packets::RejectedPackets;
use super::session::Session;
use super::session_security::SessionSecurity;
use crate::srtla::{GroupHandle, GroupRemoval, ReceiverEvent, ReceiverIo, Refusal};

/// The freewheel receiver: sessions of links, each forwarded to the SRT listener.
///
/// Feed it every datagram with the time in microseconds of a monotonic clock, and call
/// [`housekeeping`](Self::housekeeping) at least every [`ACK_MAX_DELAY_US`]. It only
/// allocates when a session is created (or, encrypted, opened again by a new handshake).
#[derive(Debug)]
pub struct Receiver {
    sessions: Vec<Session>,
    next_handle: u64,
    /// With a recording: the ranges each ACK carries. Without, background data is dropped.
    acknowledged_ranges: Option<usize>,
    /// Encrypted ([`enable_encryption`](Self::enable_encryption)): the server's keys and the
    /// boxes it accepts. `None`: wire format v0, in clear.
    keys: Option<ServerKeys>,
    /// What was dropped for the encryption's sake.
    rejected: RejectedPackets,
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
            acknowledged_ranges: None,
            keys: None,
            rejected: RejectedPackets::default(),
        }
    }

    /// Speaks wire format v1 (M1b step 7): a session opens with a Noise `IK` handshake from a
    /// box on `keys.authorized` (a hello, answered with a welcome), and every packet after it is
    /// sealed with the session's keys; anything else is dropped unanswered, v0 packets
    /// included. Without this, v0 in clear (`FREEWHEEL_PLAINTEXT=1`).
    pub fn enable_encryption(&mut self, keys: ServerKeys) {
        self.keys = Some(keys);
    }

    /// What was dropped for the encryption's sake so far.
    #[must_use]
    pub const fn rejected_packets(&self) -> RejectedPackets {
        self.rejected
    }

    /// Keeps the recording each session uploads (the background class): every chunk goes to
    /// [`ReceiverIo::write_recording`], and the chunks taken are acknowledged by sequence in
    /// every ACK of the session (`settings.acknowledged_ranges` of the latest). Without this,
    /// background data is counted, acknowledged by link and dropped.
    pub fn enable_recording(&mut self, settings: &RecordingSettings) {
        self.acknowledged_ranges = Some(settings.acknowledged_ranges);
    }

    /// A datagram from a sender's link, on the listening socket.
    pub fn handle_link_datagram(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        datagram: &[u8],
        io: &mut impl ReceiverIo,
    ) {
        if self.keys.is_none() {
            if let Ok(packet) = Packet::decode(datagram) {
                self.handle_packet(now_us, from, packet, datagram.len(), io);
            }
            return;
        }
        match Envelope::decode(datagram) {
            Ok(Envelope::Hello(hello)) => self.hello(now_us, from, &hello, io),
            Ok(Envelope::Sealed(sealed)) => {
                let mut opened = [0u8; MAX_DATAGRAM_LEN];
                let Some(plain) = self
                    .open(&sealed, &mut opened)
                    .and_then(|len| opened.get(..len))
                else {
                    return;
                };
                if let Ok(packet) = Packet::decode(plain) {
                    self.handle_packet(now_us, from, packet, datagram.len(), io);
                }
            }
            Ok(Envelope::Welcome(_)) | Err(_) => {}
        }
    }

    /// One v0 packet, read in clear or opened: `datagram_len` is what it took on the wire.
    fn handle_packet(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        packet: Packet<'_>,
        datagram_len: usize,
        io: &mut impl ReceiverIo,
    ) {
        match packet {
            Packet::Join(header) => self.join(now_us, from, header, io),
            Packet::Data(data) => self.receive_data(now_us, from, &data, datagram_len, io),
            Packet::Keepalive { header, sent_at_us } => {
                let Some((index, link)) = self.heard(now_us, from, header, io) else {
                    return;
                };
                link.latest_sent_at_us = sent_at_us;
                link.latest_arrival_us = now_us;
                link.unacknowledged_packets = 0;
                let link = *link;
                self.acknowledge(io, index, &link, now_us);
            }
            Packet::Close { header, .. } => self.close_link(now_us, header),
            Packet::Ack(_) | Packet::Accept(_) => {}
        }
    }

    /// Opens a sealed packet with its session's keys: `None` (counted) when no session has
    /// keys for it, it is a replay, or it does not authenticate.
    fn open(&mut self, sealed: &Sealed<'_>, out: &mut [u8]) -> Option<usize> {
        let security = self
            .sessions
            .iter_mut()
            .find(|session| session.id == sealed.header.session_id)
            .and_then(|session| session.security.as_deref_mut());
        let Some(security) = security else {
            self.rejected.unknown_session += 1;
            return None;
        };
        match security.open(sealed, out) {
            Ok(len) => Some(len),
            Err(OpenError::Replayed) => {
                self.rejected.replayed += 1;
                None
            }
            Err(OpenError::Forged | OpenError::BufferTooSmall) => {
                self.rejected.forged += 1;
                None
            }
        }
    }

    /// A hello: a copy of the one that opened its session is answered with the same welcome; a
    /// new one from a box on the list opens the session, or opens it again with new keys if
    /// the same box opened it before with a lower handshake number. Anything else is dropped
    /// unanswered: a box the server does not know learns nothing.
    fn hello(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        hello: &HandshakeMessage<'_>,
        io: &mut impl ReceiverIo,
    ) {
        let header = hello.header;
        let index = self
            .sessions
            .iter()
            .position(|session| session.id == header.session_id);
        let current = index
            .and_then(|index| self.sessions.get(index))
            .and_then(|session| session.security.as_deref());
        if current.is_some_and(|security| security.hello[..] == *hello.message) {
            if let Some(index) = index {
                self.welcome_link(index, now_us, from, header, io);
            }
            return;
        }
        let Some(keys) = &self.keys else {
            return;
        };
        let read = match read_hello(keys, header.session_id, hello.message) {
            Ok(read) => read,
            Err(error) => {
                self.rejected.hellos += 1;
                if let HandshakeError::UnknownBox(_) = error {
                    io.report(ReceiverEvent::LinkRefused {
                        link: from,
                        reason: Refusal::UnknownBox,
                    });
                }
                return;
            }
        };
        if let Some(current) = current {
            if current.box_key != read.box_key() {
                self.rejected.hellos += 1;
                io.report(ReceiverEvent::LinkRefused {
                    link: from,
                    reason: Refusal::SessionTaken,
                });
                return;
            }
            if read.handshake_number() <= current.handshake_number {
                self.rejected.hellos += 1;
                return;
            }
        }
        if index.is_none() && self.sessions.len() >= MAX_SESSIONS {
            io.report(ReceiverEvent::LinkRefused {
                link: from,
                reason: Refusal::TooManyGroups,
            });
            return;
        }
        let (box_key, handshake_number) = (read.box_key(), read.handshake_number());
        let mut welcome = [0u8; WELCOME_MESSAGE_LEN];
        let Ok(transport) = read.welcome(keys, &mut welcome) else {
            return;
        };
        let mut hello_bytes = [0u8; HELLO_MESSAGE_LEN];
        hello_bytes.copy_from_slice(hello.message);
        let index = match index {
            Some(index) => index,
            None => self.create_session(now_us, from, header, io),
        };
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        let previous = session.security.take().map(|security| security.transport);
        session.security = Some(Box::new(SessionSecurity {
            box_key,
            handshake_number,
            hello: hello_bytes,
            welcome,
            transport,
            previous,
        }));
        self.welcome_link(index, now_us, from, header, io);
    }

    /// Adds the link a hello came on to session `index`, and answers it with the session's
    /// welcome, in clear (it is the handshake's own message).
    fn welcome_link(
        &mut self,
        index: usize,
        now_us: u64,
        from: SocketAddrV4,
        header: Header,
        io: &mut impl ReceiverIo,
    ) {
        let Some(again) = self.add_link(index, now_us, from, header, io) else {
            return;
        };
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        let Some(security) = session.security.as_deref() else {
            return;
        };
        let welcome = Envelope::Welcome(HandshakeMessage {
            header,
            message: &security.welcome,
        });
        let mut out = [0u8; MAX_DATAGRAM_LEN];
        if let Some(datagram) = welcome.encode(&mut out).ok().and_then(|len| out.get(..len))
            && io.send_to_link(from, datagram).is_err()
        {
            io.report(ReceiverEvent::SendToLinkFailed { link: from });
        }
        io.report(ReceiverEvent::LinkRegistered {
            group: session.handle,
            link: from,
            again,
        });
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
        send_packet(
            io,
            wire(self.keys.is_some(), &mut session.security),
            address,
            &packet,
        );
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
            let mut ranges = [SequenceRange::default(); MAX_ACKNOWLEDGED_RANGES];
            let ranges = session.recent_chunks.copy_into(&mut ranges);
            let encrypted = self.keys.is_some();
            for link in &mut session.links {
                if link.unacknowledged_packets > 0
                    && now_us.saturating_sub(link.latest_arrival_us) >= ACK_MAX_DELAY_US
                {
                    let wire = wire(encrypted, &mut session.security);
                    send_ack(io, wire, session_id, link, ranges, now_us);
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
            // Encrypted, only a hello creates a session: a join comes sealed with its keys.
            None if self.keys.is_some() => return,
            None if self.sessions.len() >= MAX_SESSIONS => {
                return refuse(io, Wire::Plain, from, header, Refusal::TooManyGroups);
            }
            None => self.create_session(now_us, from, header, io),
        };
        let Some(again) = self.add_link(index, now_us, from, header, io) else {
            return;
        };
        let encrypted = self.keys.is_some();
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        let handle = session.handle;
        send_packet(
            io,
            wire(encrypted, &mut session.security),
            from,
            &Packet::Accept(header),
        );
        io.report(ReceiverEvent::LinkRegistered {
            group: handle,
            link: from,
            again,
        });
    }

    /// A new session for `header`'s session id, created by a packet from `from`; returns its
    /// index.
    fn create_session(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        header: Header,
        io: &mut impl ReceiverIo,
    ) -> usize {
        let handle = GroupHandle::new(self.next_handle);
        self.next_handle += 1;
        self.sessions.push(Session::new(
            header.session_id,
            handle,
            now_us,
            self.acknowledged_ranges.unwrap_or(1),
        ));
        io.report(ReceiverEvent::GroupRegistered {
            group: handle,
            from,
        });
        self.sessions.len() - 1
    }

    /// Adds `header`'s link to session `index` (or updates it, `Some(true)`); `None` when the
    /// session is full, which is refused.
    fn add_link(
        &mut self,
        index: usize,
        now_us: u64,
        from: SocketAddrV4,
        header: Header,
        io: &mut impl ReceiverIo,
    ) -> Option<bool> {
        let encrypted = self.keys.is_some();
        let session = self.sessions.get_mut(index)?;
        if let Some(link) = session.link_mut(header.link_id) {
            link.address = from;
            link.last_received_us = now_us;
            Some(true)
        } else if session.links.len() >= MAX_LINKS_PER_SESSION {
            refuse(
                io,
                wire(encrypted, &mut session.security),
                from,
                header,
                Refusal::GroupFull,
            );
            None
        } else {
            session.links.push(Link::new(header.link_id, from, now_us));
            Some(false)
        }
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
        let ack_due = link.unacknowledged_packets >= ACK_EVERY_PACKETS;
        if ack_due {
            link.unacknowledged_packets = 0;
        }
        let link = *link;
        // Background data is counted and acknowledged, so the sender measures the link. With a
        // recording, its chunks are kept, and acknowledged by sequence once taken (before the
        // ACK this packet may trigger); padding, and everything without a recording, goes
        // nowhere.
        if data.class == TrafficClass::Background
            && self.acknowledged_ranges.is_some()
            && let Ok(BackgroundPayload::Chunk(chunk)) = BackgroundPayload::decode(data.payload)
            && io
                .write_recording(data.header.session_id, chunk.offset, chunk.bytes)
                .is_ok()
            && let Some(session) = self.sessions.get_mut(index)
        {
            session.recent_chunks.record(data.sequence);
        }
        if ack_due {
            self.acknowledge(io, index, &link, now_us);
        }
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
        let encrypted = self.keys.is_some();
        let known = index
            .and_then(|index| self.sessions.get(index))
            .is_some_and(|session| session.links.iter().any(|link| link.id == header.link_id));
        if !known {
            let mut no_keys = None;
            let security = index
                .and_then(|index| self.sessions.get_mut(index))
                .map_or(&mut no_keys, |session| &mut session.security);
            send_packet(
                io,
                wire(encrypted, security),
                from,
                &Packet::Close {
                    header,
                    reason: CloseReason::Refused,
                },
            );
            return None;
        }
        let link = index
            .and_then(|index| self.sessions.get_mut(index))
            .and_then(|session| session.link_mut(header.link_id))?;
        link.address = from;
        link.last_received_us = now_us;
        index.map(|index| (index, link))
    }

    /// Sends `link` of session `index` an ACK, with the session's latest recording chunks.
    fn acknowledge(&mut self, io: &mut impl ReceiverIo, index: usize, link: &Link, now_us: u64) {
        let encrypted = self.keys.is_some();
        let Some(session) = self.sessions.get_mut(index) else {
            return;
        };
        let mut ranges = [SequenceRange::default(); MAX_ACKNOWLEDGED_RANGES];
        let ranges = session.recent_chunks.copy_into(&mut ranges);
        let session_id = session.id;
        send_ack(
            io,
            wire(encrypted, &mut session.security),
            session_id,
            link,
            ranges,
            now_us,
        );
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

/// How a session's packets leave: in clear, sealed with its keys, or not at all (encrypted,
/// and no keys for it).
enum Wire<'a> {
    Plain,
    Sealed(&'a mut SessionSecurity),
    NoKeys,
}

/// The wire of a session whose keys are `security`, the receiver `encrypted` or not.
fn wire(encrypted: bool, security: &mut Option<Box<SessionSecurity>>) -> Wire<'_> {
    match (encrypted, security.as_deref_mut()) {
        (false, _) => Wire::Plain,
        (true, Some(security)) => Wire::Sealed(security),
        (true, None) => Wire::NoKeys,
    }
}

/// Sends the encoded v0 packet `plain` to `to` through `wire`.
fn transmit(io: &mut impl ReceiverIo, wire: Wire<'_>, to: SocketAddrV4, plain: &[u8]) {
    let mut sealed = [0u8; MAX_DATAGRAM_LEN];
    let datagram = match wire {
        Wire::Plain => plain,
        Wire::Sealed(security) => {
            let Some(datagram) = security
                .transport
                .seal(plain, &mut sealed)
                .ok()
                .and_then(|len| sealed.get(..len))
            else {
                return;
            };
            datagram
        }
        Wire::NoKeys => return,
    };
    if io.send_to_link(to, datagram).is_err() {
        io.report(ReceiverEvent::SendToLinkFailed { link: to });
    }
}

fn send_packet(io: &mut impl ReceiverIo, wire: Wire<'_>, to: SocketAddrV4, packet: &Packet<'_>) {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    if let Some(plain) = packet.encode(&mut out).ok().and_then(|len| out.get(..len)) {
        transmit(io, wire, to, plain);
    }
}

fn send_ack(
    io: &mut impl ReceiverIo,
    wire: Wire<'_>,
    session_id: u64,
    link: &Link,
    ranges: &[SequenceRange],
    now_us: u64,
) {
    let header = Header {
        session_id,
        link_id: link.id,
    };
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    if let Some(plain) = Ack::encode(header, link.timing(now_us), ranges, &mut out)
        .ok()
        .and_then(|len| out.get(..len))
    {
        transmit(io, wire, link.address, plain);
    }
}

fn refuse(
    io: &mut impl ReceiverIo,
    wire: Wire<'_>,
    from: SocketAddrV4,
    header: Header,
    reason: Refusal,
) {
    send_packet(
        io,
        wire,
        from,
        &Packet::Close {
            header,
            reason: CloseReason::Refused,
        },
    );
    io.report(ReceiverEvent::LinkRefused { link: from, reason });
}
