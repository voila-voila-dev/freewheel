//! The sender's decisions, one datagram at a time.

use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::srtla::{
    GroupId, HALF_ID_LEN, LossRange, Packet, PacketType, REG2_LEN, SequenceNumber,
    data_sequence_number, raw_type,
};

use super::housekeeping_outcome::HousekeepingOutcome;
use super::limits::{
    GIVE_UP_AFTER_MS, HOUSEKEEPING_PERIOD_MS, IDLE_BEFORE_KEEPALIVE_S, MILLIPACKETS_PER_PACKET,
    REGISTRATION_TIMEOUT_S, WINDOW_ACK_BROADCAST_INCREASE_MILLIPACKETS,
    WINDOW_ACK_INCREASE_MILLIPACKETS, WINDOW_MAX_MILLIPACKETS, WINDOW_MIN_MILLIPACKETS,
    WINDOW_NAK_DECREASE_MILLIPACKETS,
};
use super::link::Link;
use super::link_activity::LinkActivity;
use super::link_handle::LinkHandle;
use super::link_status::LinkStatus;
use super::packet_log::PacketLog;
use super::sender_event::SenderEvent;
use super::sender_io::SenderIo;

const MILLISECONDS_PER_SECOND: u64 = 1_000;
const REG_NGP: u16 = PacketType::RegNgp.raw();
const REG2: u16 = PacketType::Reg2.raw();
const REG3: u16 = PacketType::Reg3.raw();
const SRT_ACK: u16 = PacketType::SrtAck.raw();
const SRT_NAK: u16 = PacketType::SrtNak.raw();
const SRTLA_ACK: u16 = PacketType::SrtlaAck.raw();
const KEEPALIVE: u16 = PacketType::Keepalive.raw();

/// The SRTLA sender, as upstream `srtla_send` behaves.
///
/// Feed it every datagram with the time in milliseconds of a monotonic clock, and call
/// [`housekeeping`](Self::housekeeping) at least every 200 ms (upstream's wait). Upstream
/// counts most delays in whole seconds: so does this, from the same clock. It only allocates
/// when the link list changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sender {
    group_id: GroupId,
    receivers: Vec<SocketAddrV4>,
    receiver_index: usize,
    /// In upstream's order: newest added first, which also breaks ties when choosing a link.
    links: Vec<Link>,
    next_handle: u64,
    srt_caller: Option<SocketAddrV4>,
    /// The link whose REG1 waits for its REG2.
    registering: Option<LinkHandle>,
    /// Until this second, a new REG1 is not sent (REG1 in flight, or REG3s expected).
    registration_quiet_until_s: Option<u64>,
    /// Links counted usable at the last housekeeping, plus every REG3 since.
    active_links: usize,
    has_connected: bool,
    all_failed_since_ms: Option<u64>,
    last_housekeeping_ms: Option<u64>,
}

impl Sender {
    /// A sender with no links yet, whose random group id is `group_id` (only its first half
    /// survives registration). `receivers` are the receiver's resolved addresses, tried in
    /// order until one answers; `None` when there are none.
    #[must_use]
    pub fn new(group_id: GroupId, receivers: &[SocketAddrV4]) -> Option<Self> {
        if receivers.is_empty() {
            return None;
        }
        Some(Self {
            group_id,
            receivers: receivers.to_vec(),
            receiver_index: 0,
            links: Vec::new(),
            next_handle: 0,
            srt_caller: None,
            registering: None,
            registration_quiet_until_s: None,
            active_links: 0,
            has_connected: false,
            all_failed_since_ms: None,
            last_housekeeping_ms: None,
        })
    }

    /// Makes the link list match `sources`, as upstream does at start and on `SIGHUP`: a known
    /// address keeps its link and state, a new one gets a fresh link (opened by the next
    /// housekeeping, or by [`open_links`](Self::open_links)), a missing one is closed. Returns
    /// how many links were added.
    pub fn reload_sources(&mut self, sources: &[Ipv4Addr], io: &mut impl SenderIo) -> usize {
        let mut added = 0;
        for source in sources {
            if self.links.iter().any(|link| link.source == *source) {
                continue;
            }
            let handle = LinkHandle::new(self.next_handle);
            self.next_handle += 1;
            self.links.insert(0, Link::new(handle, *source));
            io.report(SenderEvent::LinkAdded { source: *source });
            added += 1;
        }
        let registering = &mut self.registering;
        self.links.retain(|link| {
            let kept = sources.contains(&link.source);
            if !kept {
                if *registering == Some(link.handle) {
                    *registering = None;
                }
                if link.open {
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

    /// Opens every link not open yet, reporting those that fail (upstream does this once, at
    /// start). Returns how many links are open.
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

    /// A datagram from the SRT caller: sent on the best usable link, or dropped if there is
    /// none.
    pub fn handle_srt_datagram(
        &mut self,
        now_ms: u64,
        from: SocketAddrV4,
        datagram: &[u8],
        io: &mut impl SenderIo,
    ) {
        self.srt_caller = Some(from);
        let second = now_ms / MILLISECONDS_PER_SECOND;
        let Some(receiver) = self.receiver() else {
            return;
        };
        let Some(link) = self.choose_link(second) else {
            return;
        };
        link.last_chosen_s = Some(second);
        if io.send_on_link(link.handle, receiver, datagram).is_err() {
            link.activity = LinkActivity::SendFailed;
            io.report(SenderEvent::SendFailed {
                source: link.source,
            });
            return;
        }
        if let Some(number) = data_sequence_number(datagram).and_then(SequenceNumber::new) {
            link.log.push(number);
            link.in_flight_packets = link.in_flight_packets.saturating_add(1);
        }
    }

    /// A datagram from the receiver, on a link's socket.
    pub fn handle_link_datagram(
        &mut self,
        now_ms: u64,
        link: LinkHandle,
        datagram: &[u8],
        io: &mut impl SenderIo,
    ) {
        if datagram.is_empty() {
            return;
        }
        let second = now_ms / MILLISECONDS_PER_SECOND;
        let Some(index) = self.index_of(link) else {
            return;
        };
        let kind = raw_type(datagram).unwrap_or_default();
        match kind {
            REG_NGP => return self.start_registration(second, index, io),
            REG2 => return self.finish_registration(second, index, datagram, io),
            _ => {}
        }
        let Some(link) = self.links.get_mut(index) else {
            return;
        };
        link.activity = LinkActivity::ReceivedAt { second };
        match (kind, Packet::decode(datagram)) {
            (SRT_ACK, Ok(Packet::SrtAck(ack))) => self.register_srt_ack(ack.last_acknowledged()),
            (SRT_NAK, Ok(Packet::SrtNak(nak))) => {
                for range in nak.loss_ranges() {
                    self.register_loss(range);
                }
            }
            (SRTLA_ACK, decoded) => {
                if let Ok(Packet::SrtlaAck(ack)) = decoded {
                    for number in ack.sequence_numbers() {
                        self.register_srtla_ack(number);
                    }
                }
                return;
            }
            (KEEPALIVE, _) => return,
            (REG3, _) => {
                self.has_connected = true;
                self.active_links += 1;
                io.report(SenderEvent::LinkEstablished {
                    source: link.source,
                });
                return;
            }
            _ => {}
        }
        if let Some(caller) = self.srt_caller {
            let _ = io.send_to_srt(caller, datagram);
        }
    }

    /// Retries closed sockets, resets and re-registers failed links, sends keepalives on idle
    /// ones, and gives up after [`GIVE_UP_AFTER_MS`] without any link. Does nothing more often
    /// than every [`HOUSEKEEPING_PERIOD_MS`].
    pub fn housekeeping(&mut self, now_ms: u64, io: &mut impl SenderIo) -> HousekeepingOutcome {
        if self
            .last_housekeeping_ms
            .is_some_and(|last_ms| last_ms + HOUSEKEEPING_PERIOD_MS > now_ms)
        {
            return HousekeepingOutcome::Running;
        }
        let second = now_ms / MILLISECONDS_PER_SECOND;
        self.active_links = 0;
        if self.registering.is_some()
            && self
                .registration_quiet_until_s
                .is_none_or(|until_s| second > until_s)
        {
            self.registering = None;
        }
        for index in 0..self.links.len() {
            self.check_link(second, index, io);
        }
        let outcome = self.check_all_failed(now_ms, io);
        self.last_housekeeping_ms = Some(now_ms);
        outcome
    }

    /// The links at `now_ms`, in upstream's order (newest added first).
    pub fn links(&self, now_ms: u64) -> impl Iterator<Item = LinkStatus> + '_ {
        let second = now_ms / MILLISECONDS_PER_SECOND;
        self.links.iter().map(move |link| LinkStatus {
            handle: link.handle,
            source: link.source,
            open: link.open,
            usable: link.open && !link.activity.timed_out(second),
            window_millipackets: link.window_millipackets,
            in_flight_packets: link.in_flight_packets,
        })
    }

    /// The link bound to `source`.
    #[must_use]
    pub fn link_of(&self, source: Ipv4Addr) -> Option<LinkHandle> {
        self.links
            .iter()
            .find(|link| link.source == source)
            .map(|link| link.handle)
    }

    /// The group id: ours until registration, then the receiver's full one.
    #[must_use]
    pub const fn group_id(&self) -> &GroupId {
        &self.group_id
    }

    /// The receiver address in use.
    #[must_use]
    pub fn receiver(&self) -> Option<SocketAddrV4> {
        self.receivers.get(self.receiver_index).copied()
    }

    /// A REG3 has arrived at least once.
    #[must_use]
    pub const fn has_connected(&self) -> bool {
        self.has_connected
    }

    fn index_of(&self, link: LinkHandle) -> Option<usize> {
        self.links
            .iter()
            .position(|candidate| candidate.handle == link)
    }

    /// Upstream's choice: the usable link with the best score, the first one on a tie.
    fn choose_link(&mut self, now_s: u64) -> Option<&mut Link> {
        let mut best: Option<(usize, u32)> = None;
        for (index, link) in self.links.iter().enumerate() {
            if !link.open || link.activity.timed_out(now_s) {
                continue;
            }
            let score = link.score();
            if best.is_none_or(|(_, best_score)| score > best_score) {
                best = Some((index, score));
            }
        }
        best.and_then(|(index, _)| self.links.get_mut(index))
    }

    /// `REG_NGP`: the receiver does not know our group. With no usable link and no
    /// registration under way, send REG1 on that link. Not activity.
    fn start_registration(&mut self, now_s: u64, index: usize, io: &mut impl SenderIo) {
        let quiet = self
            .registration_quiet_until_s
            .is_some_and(|until_s| now_s <= until_s);
        if self.active_links != 0 || self.registering.is_some() || quiet {
            return;
        }
        let Some(link) = self.links.get(index) else {
            return;
        };
        let handle = link.handle;
        if self.send_registration(handle, PacketType::Reg1, io) {
            self.registering = Some(handle);
            self.registration_quiet_until_s = Some(now_s + REGISTRATION_TIMEOUT_S);
        }
    }

    /// REG2 on the link that sent REG1: adopt the full id and send REG2 on every link. Not
    /// activity; ignored on any other link.
    fn finish_registration(
        &mut self,
        now_s: u64,
        index: usize,
        datagram: &[u8],
        io: &mut impl SenderIo,
    ) {
        let Some(link) = self.links.get(index) else {
            return;
        };
        if self.registering != Some(link.handle) || datagram.len() != REG2_LEN {
            return;
        }
        let source = link.source;
        let Ok(Packet::Reg2(id)) = Packet::decode(datagram) else {
            return;
        };
        if id.get(..HALF_ID_LEN) != self.group_id.get(..HALF_ID_LEN) {
            io.report(SenderEvent::GroupIdMismatch { source });
            return;
        }
        self.group_id = *id;
        io.report(SenderEvent::GroupRegistered { source });
        for index in 0..self.links.len() {
            if let Some(link) = self.links.get(index).filter(|link| link.open) {
                let handle = link.handle;
                self.send_registration(handle, PacketType::Reg2, io);
            }
        }
        self.registering = None;
        self.registration_quiet_until_s = Some(now_s + REGISTRATION_TIMEOUT_S);
    }

    /// Sends REG1 or REG2 (with the current id) on a link; `false` if it did not leave.
    fn send_registration(
        &self,
        link: LinkHandle,
        kind: PacketType,
        io: &mut impl SenderIo,
    ) -> bool {
        let Some(receiver) = self.receiver() else {
            return false;
        };
        let packet = match kind {
            PacketType::Reg1 => Packet::Reg1(&self.group_id),
            _ => Packet::Reg2(&self.group_id),
        };
        let mut bytes = [0u8; REG2_LEN];
        packet
            .encode(&mut bytes)
            .ok()
            .and_then(|len| bytes.get(..len))
            .is_some_and(|datagram| io.send_on_link(link, receiver, datagram).is_ok())
    }

    /// SRT ACK of `number`: on every link, what is logged below it has arrived.
    fn register_srt_ack(&mut self, number: SequenceNumber) {
        for link in &mut self.links {
            link.in_flight_packets = link.log.forget_below(number);
        }
    }

    /// SRTLA ACK of one number: the first link that logged it has one less in flight (and
    /// grows if it was over its window); every link that ever heard back grows by one.
    fn register_srtla_ack(&mut self, number: SequenceNumber) {
        let mut found = false;
        for link in &mut self.links {
            if !found && link.log.take(number) {
                found = true;
                link.in_flight_packets = link.in_flight_packets.saturating_sub(1);
                if u64::from(link.in_flight_packets) * MILLIPACKETS_PER_PACKET
                    > u64::from(link.window_millipackets)
                {
                    link.window_millipackets = link
                        .window_millipackets
                        .saturating_add(WINDOW_ACK_INCREASE_MILLIPACKETS);
                }
            }
            if link.activity != LinkActivity::Never {
                link.window_millipackets = link
                    .window_millipackets
                    .saturating_add(WINDOW_ACK_BROADCAST_INCREASE_MILLIPACKETS)
                    .min(WINDOW_MAX_MILLIPACKETS);
            }
        }
    }

    /// SRT NAK of `range`: for each lost number, the first logged copy (links in order, newest
    /// first) is forgotten and its link's window shrinks.
    ///
    /// Upstream searches number by number. Walking the logs backwards instead gives the same
    /// result (a copy is hit exactly when no earlier copy exists) in time bounded by the logs,
    /// whatever the size of the range.
    fn register_loss(&mut self, range: LossRange) {
        let (first, last) = (range.first, range.last);
        for index in (0..self.links.len()).rev() {
            for age in PacketLog::ages().rev() {
                let Some(number) = self.links.get(index).and_then(|link| link.log.at_age(age))
                else {
                    continue;
                };
                if number < first || number > last {
                    continue;
                }
                let earlier_copy = self
                    .links
                    .get(..index)
                    .is_some_and(|earlier| earlier.iter().any(|link| link.log.holds(number)))
                    || self
                        .links
                        .get(index)
                        .is_some_and(|link| link.log.holds_younger_than(number, age));
                if earlier_copy {
                    continue;
                }
                if let Some(link) = self.links.get_mut(index) {
                    link.log.clear_age(age);
                    link.window_millipackets = link
                        .window_millipackets
                        .saturating_sub(WINDOW_NAK_DECREASE_MILLIPACKETS)
                        .max(WINDOW_MIN_MILLIPACKETS);
                }
            }
        }
    }

    /// One link at housekeeping: open it, or reset and re-register it, or count it and keep it
    /// alive.
    fn check_link(&mut self, now_s: u64, index: usize, io: &mut impl SenderIo) {
        let registering = self.registering;
        let Some(link) = self.links.get_mut(index) else {
            return;
        };
        if !link.open {
            link.open = io.open_link(link.handle, link.source).is_ok();
            return;
        }
        let handle = link.handle;
        if link.activity.timed_out(now_s) {
            if link.activity != LinkActivity::Never {
                io.report(SenderEvent::LinkFailed {
                    source: link.source,
                });
                link.activity = LinkActivity::Never;
                link.last_chosen_s = None;
                link.window_millipackets = WINDOW_MIN_MILLIPACKETS;
                link.in_flight_packets = 0;
                link.log.clear();
            }
            match registering {
                None => {
                    self.send_registration(handle, PacketType::Reg2, io);
                }
                Some(pending) if pending == handle => {
                    self.send_registration(handle, PacketType::Reg1, io);
                }
                Some(_) => {}
            }
            return;
        }
        self.active_links += 1;
        let idle = link
            .last_chosen_s
            .is_none_or(|chosen_s| chosen_s + IDLE_BEFORE_KEEPALIVE_S < now_s);
        if idle && let Some(receiver) = self.receiver() {
            let _ = io.send_on_link(handle, receiver, &KEEPALIVE.to_be_bytes());
        }
    }

    fn check_all_failed(&mut self, now_ms: u64, io: &mut impl SenderIo) -> HousekeepingOutcome {
        if self.active_links != 0 {
            self.all_failed_since_ms = None;
            return HousekeepingOutcome::Running;
        }
        let since_ms = *self.all_failed_since_ms.get_or_insert(now_ms);
        if self.has_connected {
            io.report(SenderEvent::NoLinkAvailable);
        }
        if now_ms <= since_ms + GIVE_UP_AFTER_MS {
            return HousekeepingOutcome::Running;
        }
        if !self.has_connected
            && let Some(next) = self.receivers.get(self.receiver_index + 1).copied()
        {
            self.receiver_index += 1;
            self.all_failed_since_ms = None;
            io.report(SenderEvent::TryingReceiver { receiver: next });
            return HousekeepingOutcome::Running;
        }
        io.report(SenderEvent::GaveUp {
            had_connected: self.has_connected,
        });
        HousekeepingOutcome::GaveUp
    }
}
