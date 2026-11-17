//! The freewheel sender's decisions, one datagram at a time.

use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::freewheel::{
    CloseReason, Data, Header, MAX_DATA_PAYLOAD_LEN, MAX_DATAGRAM_LEN, Packet, SessionId,
    TrafficClass,
};

use super::limits::{
    HOUSEKEEPING_PERIOD_US, JOIN_RETRY_US, KEEPALIVE_IDLE_US, MAX_EXPECTED_ARRIVAL_US,
    OPEN_RETRY_US, STALL_MAX_US, STALL_MIN_US, STALL_RTT_MULTIPLE,
};
use super::link::Link;
use super::link_estimate::wrapped_us;
use crate::srtla::{LinkHandle, SenderEvent, SenderIo};

/// What the outside world may know about one link (for the event loop and tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreewheelLinkStatus {
    /// The link.
    pub handle: LinkHandle,
    /// Its source address.
    pub source: Ipv4Addr,
    /// It may carry data now.
    pub usable: bool,
    /// Smoothed RTT, in microseconds.
    pub smoothed_rtt_us: u64,
    /// Estimated delivery rate, in bit/s.
    pub rate_bps: u64,
    /// Bytes sent and not acknowledged.
    pub in_flight_bytes: u64,
}

/// The freewheel sender: one session, spread over every link by expected arrival time.
///
/// Feed it every datagram with the time in microseconds of a monotonic clock, and call
/// [`housekeeping`](Self::housekeeping) at least every [`HOUSEKEEPING_PERIOD_US`]. It only
/// allocates when the link list changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sender {
    session_id: SessionId,
    receiver: SocketAddrV4,
    links: Vec<Link>,
    next_handle: u64,
    next_link_id: u8,
    srt_caller: Option<SocketAddrV4>,
    next_sequence: u32,
    last_housekeeping_us: Option<u64>,
}

impl Sender {
    /// A sender with no links yet, for the session `session_id` (random, from the box).
    #[must_use]
    pub const fn new(session_id: SessionId, receiver: SocketAddrV4) -> Self {
        Self {
            session_id,
            receiver,
            links: Vec::new(),
            next_handle: 0,
            next_link_id: 0,
            srt_caller: None,
            next_sequence: 0,
            last_housekeeping_us: None,
        }
    }

    /// Makes the link list match `sources`: a known address keeps its link, a new one gets
    /// one (opened by the next housekeeping, or [`open_links`](Self::open_links)), a missing
    /// one is closed. Returns how many links were added.
    pub fn reload_sources(&mut self, sources: &[Ipv4Addr], io: &mut impl SenderIo) -> usize {
        let mut added = 0;
        for source in sources {
            if self.links.iter().any(|link| link.source == *source) {
                continue;
            }
            let handle = LinkHandle::new(self.next_handle);
            self.next_handle += 1;
            self.links
                .push(Link::new(handle, *source, self.next_link_id));
            self.next_link_id = self.next_link_id.wrapping_add(1);
            io.report(SenderEvent::LinkAdded { source: *source });
            added += 1;
        }
        let (session_id, receiver) = (self.session_id, self.receiver);
        self.links.retain(|link| {
            let kept = sources.contains(&link.source);
            if !kept {
                if link.open {
                    let close = Packet::Close {
                        header: Header {
                            session_id,
                            link_id: link.id,
                        },
                        reason: CloseReason::Done,
                    };
                    send_packet(io, link.handle, receiver, &close);
                    io.close_link(link.handle);
                }
                io.report(SenderEvent::LinkRemoved {
                    source: link.source,
                });
            }
            kept
        });
        added
    }

    /// Opens every link not open yet. Returns how many links are open.
    pub fn open_links(&mut self, io: &mut impl SenderIo) -> usize {
        for link in self.links.iter_mut().filter(|link| !link.open) {
            link.open = io.open_link(link.handle, link.source).is_ok();
            if !link.open {
                io.report(SenderEvent::LinkNotOpened {
                    source: link.source,
                });
            }
        }
        self.links.iter().filter(|link| link.open).count()
    }

    /// A datagram from the SRT caller: sent on the link where it should arrive first, or
    /// dropped if no link is up or every link would deliver it too late.
    pub fn handle_srt_datagram(
        &mut self,
        now_us: u64,
        from: SocketAddrV4,
        datagram: &[u8],
        io: &mut impl SenderIo,
    ) {
        self.srt_caller = Some(from);
        if datagram.len() > MAX_DATA_PAYLOAD_LEN {
            return;
        }
        let size_bytes = (datagram.len() + MAX_DATAGRAM_LEN - MAX_DATA_PAYLOAD_LEN) as u64;
        let Some(index) = self.choose_link(size_bytes) else {
            return;
        };
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let (session_id, receiver) = (self.session_id, self.receiver);
        let Some(link) = self.links.get_mut(index) else {
            return;
        };
        let packet = Packet::Data(Data {
            header: Header {
                session_id,
                link_id: link.id,
            },
            class: TrafficClass::Live,
            sequence,
            sent_at_us: wrapped_us(now_us),
            payload: datagram,
        });
        if !send_packet(io, link.handle, receiver, &packet) {
            stall(
                link,
                io,
                SenderEvent::SendFailed {
                    source: link.source,
                },
            );
            return;
        }
        if link.in_flight.is_empty() {
            link.last_ack_us = link.last_ack_us.max(now_us);
        }
        link.last_sent_us = now_us;
        link.in_flight
            .push(sequence, u16::try_from(size_bytes).unwrap_or(u16::MAX));
    }

    /// A datagram from the receiver, on a link's socket.
    pub fn handle_link_datagram(
        &mut self,
        now_us: u64,
        link: LinkHandle,
        datagram: &[u8],
        io: &mut impl SenderIo,
    ) {
        let Ok(packet) = Packet::decode(datagram) else {
            return;
        };
        if packet.header().session_id != self.session_id {
            return;
        }
        match packet {
            Packet::Data(data) => {
                if let Some(caller) = self.srt_caller {
                    let _ = io.send_to_srt(caller, data.payload);
                }
            }
            Packet::Accept(header) => {
                if let Some(link) = self.link_by_id(header.link_id) {
                    accept(link, now_us, io);
                }
            }
            Packet::Ack(ack) => {
                let Some(link) = self.link_by_id(ack.header.link_id) else {
                    return;
                };
                if !link.joined || link.stalled {
                    accept(link, now_us, io);
                }
                let left_flight_bytes = link.in_flight.acknowledge(ack.timing.largest_sequence);
                link.estimate.on_ack(now_us, ack.timing, left_flight_bytes);
                link.last_ack_us = now_us;
            }
            Packet::Close { .. } => {
                if let Some(link) = self.links.iter_mut().find(|l| l.handle == link) {
                    link.joined = false;
                    link.last_join_us = None;
                    link.in_flight.clear();
                }
            }
            Packet::Keepalive { .. } | Packet::Join(_) => {}
        }
    }

    /// Opens closed sockets, joins links not accepted yet, stalls links that stopped
    /// acknowledging, sends keepalives on idle ones.
    pub fn housekeeping(&mut self, now_us: u64, io: &mut impl SenderIo) {
        if self
            .last_housekeeping_us
            .is_some_and(|last_us| last_us + HOUSEKEEPING_PERIOD_US > now_us)
        {
            return;
        }
        self.last_housekeeping_us = Some(now_us);
        let (session_id, receiver) = (self.session_id, self.receiver);
        for link in &mut self.links {
            let header = Header {
                session_id,
                link_id: link.id,
            };
            if !link.open {
                if link
                    .last_open_attempt_us
                    .is_none_or(|at_us| at_us + OPEN_RETRY_US <= now_us)
                {
                    link.last_open_attempt_us = Some(now_us);
                    link.open = io.open_link(link.handle, link.source).is_ok();
                }
                continue;
            }
            if !link.joined {
                if link
                    .last_join_us
                    .is_none_or(|at_us| at_us + JOIN_RETRY_US <= now_us)
                {
                    link.last_join_us = Some(now_us);
                    send_packet(io, link.handle, receiver, &Packet::Join(header));
                }
                continue;
            }
            let stall_after_us = (STALL_RTT_MULTIPLE * link.estimate.smoothed_rtt_us())
                .clamp(STALL_MIN_US, STALL_MAX_US);
            if !link.stalled
                && link.last_sent_us > link.last_ack_us
                && now_us.saturating_sub(link.last_ack_us) > stall_after_us
            {
                stall(
                    link,
                    io,
                    SenderEvent::LinkFailed {
                        source: link.source,
                    },
                );
            }
            if now_us.saturating_sub(link.last_sent_us) >= KEEPALIVE_IDLE_US {
                let keepalive = Packet::Keepalive {
                    header,
                    sent_at_us: wrapped_us(now_us),
                };
                if send_packet(io, link.handle, receiver, &keepalive) {
                    link.last_sent_us = now_us;
                }
            }
        }
    }

    /// The links, in the order they were added.
    pub fn links(&self) -> impl Iterator<Item = FreewheelLinkStatus> + '_ {
        self.links.iter().map(|link| FreewheelLinkStatus {
            handle: link.handle,
            source: link.source,
            usable: link.usable(),
            smoothed_rtt_us: link.estimate.smoothed_rtt_us(),
            rate_bps: link.estimate.rate_bps(),
            in_flight_bytes: link.in_flight.bytes(),
        })
    }

    /// The session id.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// The receiver's address.
    #[must_use]
    pub const fn receiver(&self) -> SocketAddrV4 {
        self.receiver
    }

    fn link_by_id(&mut self, id: u8) -> Option<&mut Link> {
        self.links.iter_mut().find(|link| link.id == id)
    }

    /// The usable link where a packet of `size_bytes` should arrive first, preferring links
    /// within their congestion window; when every link is full, the earliest anyway, unless
    /// even that one would deliver it after [`MAX_EXPECTED_ARRIVAL_US`]: then none.
    fn choose_link(&self, size_bytes: u64) -> Option<usize> {
        let mut best_within: Option<(usize, u64)> = None;
        let mut best_any: Option<(usize, u64)> = None;
        for (index, link) in self.links.iter().enumerate() {
            if !link.usable() {
                continue;
            }
            let in_flight_bytes = link.in_flight.bytes();
            let arrival_us = link
                .estimate
                .expected_arrival_us(in_flight_bytes, size_bytes);
            let better = |best: Option<(usize, u64)>| best.is_none_or(|(_, at)| arrival_us < at);
            if better(best_any) {
                best_any = Some((index, arrival_us));
            }
            let within = in_flight_bytes + size_bytes <= link.estimate.congestion_window_bytes();
            if within && better(best_within) {
                best_within = Some((index, arrival_us));
            }
        }
        best_within
            .or(best_any)
            .filter(|(_, arrival_us)| *arrival_us <= MAX_EXPECTED_ARRIVAL_US)
            .map(|(index, _)| index)
    }
}

fn accept(link: &mut Link, now_us: u64, io: &mut impl SenderIo) {
    if !link.joined || link.stalled {
        io.report(SenderEvent::LinkEstablished {
            source: link.source,
        });
    }
    link.joined = true;
    link.stalled = false;
    link.last_ack_us = now_us;
}

fn stall(link: &mut Link, io: &mut impl SenderIo, event: SenderEvent) {
    link.stalled = true;
    link.in_flight.clear();
    link.estimate.on_stall();
    io.report(event);
}

/// Encodes and sends one packet on a link; `false` if it did not leave whole.
fn send_packet(
    io: &mut impl SenderIo,
    link: LinkHandle,
    receiver: SocketAddrV4,
    packet: &Packet<'_>,
) -> bool {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    packet
        .encode(&mut out)
        .ok()
        .and_then(|len| out.get(..len))
        .is_some_and(|datagram| io.send_on_link(link, receiver, datagram).is_ok())
}
