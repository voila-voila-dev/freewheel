//! The freewheel sender's decisions, one datagram at a time.

use std::net::{Ipv4Addr, SocketAddrV4};

use bitrate_control::CapacityReport;
use bonding_protocol::freewheel::{
    BackgroundPayload, CloseReason, DATA_HEADER_LEN, Data, Header, MAX_DATA_PAYLOAD_LEN,
    MAX_DATAGRAM_LEN, MAX_RECORDING_CHUNK_LEN, Packet, RecordingChunk, SessionId, TrafficClass,
};

use super::limits::{
    HOUSEKEEPING_PERIOD_US, JOIN_RETRY_US, KEEPALIVE_IDLE_US, MAX_EXPECTED_ARRIVAL_US,
    MAX_OVERFLOW_ARRIVAL_US, OPEN_RETRY_US, STALL_MAX_US, STALL_MIN_US, STALL_RTT_MULTIPLE,
};
use super::link::Link;
use super::link_estimate::wrapped_us;
use super::live_activity::LiveActivity;
use super::probe::{PROBE_GAIN, ProbeTarget, Probing};
use super::recording_settings::RecordingSettings;
use super::recording_upload::RecordingUpload;
use super::spare_window::SpareWindowSettings;
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
    /// Since the last [`capacity_report`](Self::capacity_report): live bytes sent, sent to
    /// arrive late (queuing), and dropped at the box.
    traffic: Traffic,
    /// Capacity probing, once [`enable_probing`](Self::enable_probing) has been called.
    probing: Option<Probing>,
    /// Padding bytes sent so far.
    padding_bytes: u64,
    /// The recording upload, once [`enable_recording`](Self::enable_recording) has been called.
    recording: Option<RecordingUpload>,
    /// What the logs were told of the upload: it ended, it finished (delivered or abandoned).
    recording_ended_reported: bool,
    recording_finished_reported: bool,
    /// The upload's share of a link's spare room, in per cent, and the queue beyond which it
    /// sends nothing there, in microseconds ([`RecordingSettings`]).
    spare_share_percent: u64,
    spare_queue_target_us: u64,
    spare_base_rtt_window_us: u64,
    spare_gain_bytes: u64,
    spare_idle_live_bps: u64,
    /// The live traffic of the last second, for the upload's queue target.
    live_activity: LiveActivity,
}

/// Live bytes counted since the last capacity report, and when that was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Traffic {
    since_us: Option<u64>,
    sent_bytes: u64,
    overflow_bytes: u64,
    dropped_bytes: u64,
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
            traffic: Traffic {
                since_us: None,
                sent_bytes: 0,
                overflow_bytes: 0,
                dropped_bytes: 0,
            },
            probing: None,
            padding_bytes: 0,
            recording: None,
            recording_ended_reported: false,
            recording_finished_reported: false,
            spare_share_percent: 0,
            spare_queue_target_us: 0,
            spare_base_rtt_window_us: 0,
            spare_gain_bytes: 0,
            spare_idle_live_bps: 0,
            live_activity: LiveActivity::new(),
        }
    }

    /// Uploads the recording `io` reads ([`SenderIo::recording_bytes`],
    /// [`SenderIo::read_recording`]) in the background class (`recording_upload.rs`): its
    /// chunks take the place of capacity probing's padding, and fill the room the live
    /// traffic leaves in each link's congestion window
    /// ([`fill_spare_capacity`](Self::fill_spare_capacity)).
    pub fn enable_recording(&mut self, settings: &RecordingSettings) {
        self.recording = Some(RecordingUpload::new(settings));
        self.spare_share_percent = settings.share_percent;
        self.spare_queue_target_us = settings.queue_target_us;
        self.spare_base_rtt_window_us = settings.base_rtt_window_us;
        self.spare_gain_bytes = settings.window_gain_chunks * settings.chunk_bytes as u64;
        self.spare_idle_live_bps = settings.idle_live_bps;
    }

    /// The recording upload, if enabled.
    #[must_use]
    pub const fn recording(&self) -> Option<&RecordingUpload> {
        self.recording.as_ref()
    }

    /// Probes the links' spare capacity with padding while the encoder is below its maximum
    /// and the links are not known to carry what it needs (`probe.rs`). Needs the
    /// [`capacity_report`](Self::capacity_report)s, which tell it the live traffic.
    pub const fn enable_probing(&mut self, target: ProbeTarget) {
        self.probing = Some(Probing::new(target));
    }

    /// Padding bytes sent so far, on every link.
    #[must_use]
    pub const fn padding_bytes(&self) -> u64 {
        self.padding_bytes
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
        let recording = &mut self.recording;
        self.links.retain(|link| {
            let kept = sources.contains(&link.source);
            if !kept {
                if let Some(upload) = recording.as_mut() {
                    upload.on_link_lost(link.id);
                }
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
        let Some((index, arrival_us)) = self.choose_link(now_us, size_bytes) else {
            self.traffic.dropped_bytes += size_bytes;
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
                now_us,
                io,
                SenderEvent::SendFailed {
                    source: link.source,
                },
                self.recording.as_mut(),
            );
            return;
        }
        if link.in_flight.is_empty() {
            link.last_ack_us = link.last_ack_us.max(now_us);
        }
        link.last_sent_us = now_us;
        link.sent_bytes += size_bytes;
        link.in_flight
            .push(sequence, u16::try_from(size_bytes).unwrap_or(u16::MAX));
        self.traffic.sent_bytes += size_bytes;
        self.live_activity.on_sent(now_us, size_bytes);
        if arrival_us > QUEUING_ARRIVAL_US {
            self.traffic.overflow_bytes += size_bytes;
        }
    }

    /// What the links carried since the last call, for the encoder's bitrate
    /// (`bitrate-control`): what the usable links are estimated to carry (probing included),
    /// and the live data sent, sent to
    /// arrive after [`QUEUING_ARRIVAL_US`] (queuing: a window full is not enough, the stream's
    /// bursts fill them on links with room to spare) and dropped, as rates over the period. The
    /// first call only starts the period.
    pub fn capacity_report(&mut self, now_us: u64) -> CapacityReport {
        let (usable_links, capacity_bps) = self
            .links
            .iter()
            .filter(|link| link.usable())
            .fold((0_u8, 0_u64), |(count, sum), link| {
                (count.saturating_add(1), sum + link.capacity_bps(now_us))
            });
        let traffic = core::mem::take(&mut self.traffic);
        self.traffic.since_us = Some(now_us);
        let period_us = traffic
            .since_us
            .map_or(0, |since_us| now_us.saturating_sub(since_us));
        let rate = |bytes: u64| {
            if period_us == 0 {
                0
            } else {
                bytes * BITS_PER_BYTE * MICROSECONDS_PER_SECOND / period_us
            }
        };
        if let Some(probing) = &mut self.probing
            && period_us > 0
        {
            probing.on_live(rate(traffic.sent_bytes));
        }
        CapacityReport {
            capacity_bps,
            sent_bps: rate(traffic.sent_bytes),
            overflow_bps: rate(traffic.overflow_bytes),
            dropped_bps: rate(traffic.dropped_bytes),
            usable_links,
        }
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
                if let Some(upload) = &mut self.recording
                    && self.links.iter().any(|link| link.id == ack.header.link_id)
                {
                    upload.on_acknowledged(ack.ranges.iter());
                    upload.on_link_acknowledged(ack.header.link_id, ack.timing.largest_sequence);
                }
                let base_rtt_window_us = self
                    .recording
                    .is_some()
                    .then_some(self.spare_base_rtt_window_us);
                let live_idle = self.live_activity.idle(now_us, self.spare_idle_live_bps);
                let (spare_share_percent, queue_target_us, spare_gain_bytes) = (
                    self.spare_share_percent,
                    self.spare_queue_target_us,
                    self.spare_gain_bytes,
                );
                let Some(link) = self.link_by_id(ack.header.link_id) else {
                    return;
                };
                if !link.joined || link.stalled {
                    accept(link, now_us, io);
                }
                let left_flight_bytes = link.in_flight.acknowledge(ack.timing.largest_sequence);
                link.estimate.on_ack(now_us, ack.timing, left_flight_bytes);
                link.probe.on_ack(
                    now_us,
                    ack.timing.largest_sequence,
                    ack.timing.received_bytes,
                );
                link.last_ack_us = now_us;
                let left_spare_bytes = link.in_flight.take_left_spare_bytes();
                if let Some(window_us) = base_rtt_window_us {
                    link.base_rtt
                        .observe(now_us, link.estimate.smoothed_rtt_us(), window_us);
                    if left_spare_bytes > 0 {
                        let queue_us = link
                            .base_rtt
                            .queuing_delay_us(link.estimate.smoothed_rtt_us())
                            .unwrap_or(0);
                        let settings = SpareWindowSettings {
                            queue_target_us: spare_queue_target_us(
                                link,
                                now_us,
                                queue_target_us,
                                live_idle,
                            ),
                            gain_bytes: spare_gain_bytes,
                            min_bytes: MIN_SPARE_WINDOW_BYTES,
                            max_bytes: spare_cap_bytes(link, now_us, spare_share_percent),
                        };
                        link.spare_window.on_acknowledged(
                            now_us,
                            left_spare_bytes,
                            queue_us,
                            link.estimate.smoothed_rtt_us(),
                            &settings,
                        );
                    }
                }
                // The ACK freed room on that link: the upload's chunks are clocked by them.
                self.fill_spare_capacity(now_us, io);
            }
            Packet::Close { .. } => {
                if let Some(link) = self.links.iter_mut().find(|l| l.handle == link) {
                    link.joined = false;
                    link.last_join_us = None;
                    link.in_flight.clear();
                    if let Some(upload) = &mut self.recording {
                        upload.on_link_lost(link.id);
                    }
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
                    now_us,
                    io,
                    SenderEvent::LinkFailed {
                        source: link.source,
                    },
                    self.recording.as_mut(),
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
        self.follow_recording(now_us, io);
        self.probe(now_us, io);
        self.fill_spare_capacity(now_us, io);
    }

    /// Sends the recording's chunks on the links' spare room, one at a time to the link whose
    /// spare window is the least full: on a
    /// usable, measured link the live traffic has not found full lately, outside probe runs, the
    /// upload's bytes in flight stay within the link's spare window (`spare_window.rs`: widened
    /// by each ACK while the queue is under the target, narrowed beyond) and within the share of
    /// what the live traffic leaves of the congestion window
    /// ([`spare_cap_bytes`]). The live traffic does not count them
    /// ([`InFlight::live_view_bytes`](super::InFlight::live_view_bytes)): they yield to it.
    /// Never padding: spare room carries the recording or nothing.
    fn fill_spare_capacity(&mut self, now_us: u64, io: &mut impl SenderIo) {
        let Some(upload) = &mut self.recording else {
            return;
        };
        let (session_id, receiver) = (self.session_id, self.receiver);
        let live_idle = self.live_activity.idle(now_us, self.spare_idle_live_bps);
        let window_settings = |link: &Link| SpareWindowSettings {
            queue_target_us: spare_queue_target_us(
                link,
                now_us,
                self.spare_queue_target_us,
                live_idle,
            ),
            gain_bytes: self.spare_gain_bytes,
            min_bytes: MIN_SPARE_WINDOW_BYTES,
            max_bytes: spare_cap_bytes(link, now_us, self.spare_share_percent),
        };
        // One chunk at a time, to the link whose spare window is the least full: on alike links
        // they carry alike shares, so what the live traffic measures of them stays alike.
        loop {
            let best = self
                .links
                .iter()
                .enumerate()
                .filter(|(_, link)| {
                    link.usable()
                        && link.estimate.has_rtt()
                        && !link.probe.busy()
                        && !link.proven_full(now_us)
                })
                .filter_map(|(index, link)| {
                    let room_bytes = link.spare_window.bytes(&window_settings(link));
                    let used_bytes = link.in_flight.spare_bytes();
                    (used_bytes + PADDING_DATAGRAM_BYTES <= room_bytes)
                        .then(|| (index, used_bytes * PER_MILLE / room_bytes.max(1)))
                })
                .min_by_key(|(_, used_per_mille)| *used_per_mille)
                .map(|(index, _)| index);
            let Some(link) = best.and_then(|index| self.links.get_mut(index)) else {
                return;
            };
            let sequence = self.next_sequence;
            let header = Header {
                session_id,
                link_id: link.id,
            };
            let sent = match send_chunk(io, link.handle, receiver, header, sequence, now_us, upload)
            {
                ChunkSent::Sent(sent) => sent,
                ChunkSent::NothingToSend => return,
                ChunkSent::Failed => {
                    stall(
                        link,
                        now_us,
                        io,
                        SenderEvent::SendFailed {
                            source: link.source,
                        },
                        Some(upload),
                    );
                    continue;
                }
            };
            self.next_sequence = sequence.wrapping_add(1);
            if link.in_flight.is_empty() {
                link.last_ack_us = link.last_ack_us.max(now_us);
            }
            link.last_sent_us = now_us;
            link.sent_bytes += sent;
            link.in_flight
                .push_spare(sequence, u16::try_from(sent).unwrap_or(u16::MAX));
        }
    }

    /// Reads how long the recording is, finds lost chunks by their timeout, and tells the logs
    /// when the recording ended and when the upload finished.
    fn follow_recording(&mut self, now_us: u64, io: &mut impl SenderIo) {
        let Some(upload) = &mut self.recording else {
            return;
        };
        if let Some(bytes) = io.recording_bytes() {
            upload.on_recording_bytes(now_us, bytes);
        }
        upload.expire(now_us);
        if upload.ended() && !self.recording_ended_reported {
            self.recording_ended_reported = true;
            io.report(SenderEvent::RecordingEnded {
                bytes: upload.recording_bytes(),
            });
        }
        if !upload.ended() {
            self.recording_ended_reported = false;
            self.recording_finished_reported = false;
        } else if !self.recording_finished_reported {
            if upload.complete() {
                self.recording_finished_reported = true;
                io.report(SenderEvent::RecordingDelivered {
                    bytes: upload.recording_bytes(),
                    sent_chunks: upload.sent_chunks(),
                    lost_chunks: upload.lost_chunks(),
                });
            } else if upload.abandoned(now_us) {
                self.recording_finished_reported = true;
                io.report(SenderEvent::RecordingAbandoned {
                    delivered_bytes: upload.delivered_bytes(),
                    recording_bytes: upload.recording_bytes(),
                });
            }
        }
    }

    /// Starts runs of padding on the links due for one, and paces the runs going on
    /// (`probe.rs`).
    fn probe(&mut self, now_us: u64, io: &mut impl SenderIo) {
        let Some(mut probing) = self.probing else {
            return;
        };
        let capacity_bps: u64 = self
            .links
            .iter()
            .filter(|link| link.usable())
            .map(|link| link.capacity_bps(now_us))
            .sum();
        probing.refill(now_us, capacity_bps);
        let below_maximum = probing.below_maximum();
        let wanted = probing.wanted(capacity_bps);
        let wanted_capacity_bps = probing.target.wanted_capacity_bps;
        // An upload with a backlog fills every link's spare room up to its window, and what a
        // link delivers then is what it carries: a probe run would only stop it for the run
        // and its settling (a third of every second), so runs wait for the backlog to clear.
        let uploading = self
            .recording
            .as_ref()
            .is_some_and(|upload| upload.has_backlog(now_us));
        let (session_id, receiver) = (self.session_id, self.receiver);
        for link in &mut self.links {
            link.probe.advance(now_us);
            if !link.usable() {
                continue;
            }
            if link.probe.run.is_none() {
                // A link back from a stall is probed at the rate it had, whatever the others.
                let returning = link.probe.known_bps.is_some();
                if !below_maximum
                    || !((wanted && !uploading) || returning)
                    || !link.estimate.has_rtt()
                    || link.proven_full(now_us)
                    || !link.probe.due(now_us)
                {
                    continue;
                }
                let carried_bps = link.capacity_bps(now_us);
                let target_bps = link
                    .probe
                    .known_bps
                    .unwrap_or(0)
                    .max(carried_bps * PROBE_GAIN)
                    .min(wanted_capacity_bps);
                if target_bps <= carried_bps {
                    link.probe.known_bps = None;
                    continue;
                }
                // The whole run or none: a run cut short measures less than the link carries.
                let cost_bytes = Probing::run_cost_bytes(target_bps, carried_bps);
                if probing.budget_bytes < cost_bytes {
                    continue;
                }
                link.probe.start(now_us, target_bps, link.sent_bytes);
            }
            let Some(run) = link.probe.run else {
                continue;
            };
            let window_bytes = link.estimate.congestion_window_bytes(run.target_bps);
            while link.probe.owed_bytes(now_us, link.sent_bytes) >= PADDING_DATAGRAM_BYTES
                && probing.budget_bytes >= PADDING_DATAGRAM_BYTES
                && link.in_flight.bytes() + PADDING_DATAGRAM_BYTES <= window_bytes
            {
                let sequence = self.next_sequence;
                let header = Header {
                    session_id,
                    link_id: link.id,
                };
                let Some(sent) = send_background(
                    io,
                    link.handle,
                    receiver,
                    header,
                    sequence,
                    now_us,
                    self.recording.as_mut(),
                ) else {
                    stall(
                        link,
                        now_us,
                        io,
                        SenderEvent::SendFailed {
                            source: link.source,
                        },
                        self.recording.as_mut(),
                    );
                    break;
                };
                self.next_sequence = sequence.wrapping_add(1);
                link.probe.on_padding_sent(sequence);
                if link.in_flight.is_empty() {
                    link.last_ack_us = link.last_ack_us.max(now_us);
                }
                link.last_sent_us = now_us;
                link.sent_bytes += sent.size_bytes;
                link.in_flight
                    .push(sequence, u16::try_from(sent.size_bytes).unwrap_or(u16::MAX));
                probing.budget_bytes = probing.budget_bytes.saturating_sub(sent.size_bytes);
                if sent.padding {
                    self.padding_bytes += sent.size_bytes;
                }
            }
        }
        self.probing = Some(probing);
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
    /// even that one would deliver it after [`MAX_OVERFLOW_ARRIVAL_US`]: then none. A link
    /// that has not filled its window lately counts at least the average rate of the usable
    /// links ([`WINDOW_FULL_MEMORY_US`](super::limits::WINDOW_FULL_MEMORY_US)); one found full is remembered as such. Also says
    /// when the packet should arrive, in microseconds from now.
    fn choose_link(&mut self, now_us: u64, size_bytes: u64) -> Option<(usize, u64)> {
        let (usable, rate_sum_bps) = self
            .links
            .iter()
            .filter(|link| link.usable())
            .fold((0_u64, 0_u64), |(count, sum), link| {
                (count + 1, sum + link.estimate.rate_bps())
            });
        let average_rate_bps = rate_sum_bps / usable.max(1);
        let mut best_within: Option<(usize, u64)> = None;
        let mut best_any: Option<(usize, u64)> = None;
        for (index, link) in self.links.iter_mut().enumerate() {
            if !link.usable() {
                continue;
            }
            let proven_full = link.proven_full(now_us);
            let rate_bps = if proven_full {
                link.estimate.rate_bps()
            } else {
                link.estimate.rate_bps().max(average_rate_bps)
            };
            // The recording's bytes on spare room yield to the live traffic: not counted here.
            let in_flight_bytes = link.in_flight.live_view_bytes();
            let arrival_us =
                link.estimate
                    .expected_arrival_us(in_flight_bytes, size_bytes, rate_bps);
            let better = |best: Option<(usize, u64)>| best.is_none_or(|(_, at)| arrival_us < at);
            if better(best_any) {
                best_any = Some((index, arrival_us));
            }
            let within =
                in_flight_bytes + size_bytes <= link.estimate.congestion_window_bytes(rate_bps);
            // Padding fills a probed link's window on purpose: that says nothing of its capacity.
            if !within && !link.probe.busy() {
                link.last_window_full_us = Some(now_us);
                link.probe.forget_probed();
            }
            if within && better(best_within) {
                best_within = Some((index, arrival_us));
            }
        }
        best_within
            .filter(|(_, arrival_us)| *arrival_us <= MAX_EXPECTED_ARRIVAL_US)
            .or_else(|| best_any.filter(|(_, arrival_us)| *arrival_us <= MAX_OVERFLOW_ARRIVAL_US))
    }
}

/// A packet expected to arrive later than this is queuing in a modem: the links carry less
/// than the stream (the capacity report's `overflow_bps`). An idle link's RTT is 40–160 ms in
/// the lab.
pub const QUEUING_ARRIVAL_US: u64 = 250_000;
const BITS_PER_BYTE: u64 = 8;
const PERCENT: u64 = 100;
const PER_MILLE: u64 = 1_000;
/// The receiver acknowledges every this many data packets (`bonding_receiver::freewheel`'s
/// `ACK_EVERY_PACKETS`): an RTT sample is the last of them, sent behind the others.
const ACK_EVERY_PACKETS: u32 = 4;
/// Jitter read as a queue: twice the RTT variation (most samples lie within it).
const JITTER_ALLOWANCE: u64 = 2;
/// The recording upload's smallest window on a link: two full datagrams, so it keeps the link
/// measured however full its queue.
const MIN_SPARE_WINDOW_BYTES: u64 = 2 * MAX_DATAGRAM_LEN as u64;

const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
/// One padding datagram: the largest the wire carries.
const PADDING_DATAGRAM_BYTES: u64 = MAX_DATAGRAM_LEN as u64;
static PADDING: [u8; MAX_DATA_PAYLOAD_LEN] = [0; MAX_DATA_PAYLOAD_LEN];

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

/// What sending a chunk of the recording came to.
enum ChunkSent {
    /// It left: its size on the wire, in bytes.
    Sent(u64),
    /// No chunk is ready, or it cannot be read.
    NothingToSend,
    /// The send failed.
    Failed,
}

/// Sends the recording's next chunk as `sequence`, if one is ready and can be read.
fn send_chunk(
    io: &mut impl SenderIo,
    link: LinkHandle,
    receiver: SocketAddrV4,
    header: Header,
    sequence: u32,
    now_us: u64,
    upload: &mut RecordingUpload,
) -> ChunkSent {
    let mut bytes = [0u8; MAX_RECORDING_CHUNK_LEN];
    let mut payload = [0u8; MAX_DATA_PAYLOAD_LEN];
    let Some(chunk) = upload.next_chunk(now_us) else {
        return ChunkSent::NothingToSend;
    };
    let Some(bytes) = bytes.get_mut(..chunk.len) else {
        return ChunkSent::NothingToSend;
    };
    if io.read_recording(chunk.offset, bytes).is_err() {
        return ChunkSent::NothingToSend;
    }
    let Ok(len) = BackgroundPayload::Chunk(RecordingChunk {
        offset: chunk.offset,
        bytes,
    })
    .encode(&mut payload) else {
        return ChunkSent::NothingToSend;
    };
    let body = payload.get(..len).unwrap_or_default();
    let packet = Packet::Data(Data {
        header,
        class: TrafficClass::Background,
        sequence,
        sent_at_us: wrapped_us(now_us),
        payload: body,
    });
    if !send_packet(io, link, receiver, &packet) {
        return ChunkSent::Failed;
    }
    upload.on_sent(chunk, sequence, header.link_id, now_us);
    ChunkSent::Sent((DATA_HEADER_LEN + body.len()) as u64)
}

/// One background datagram that left: its size on the wire, and whether it was padding.
struct BackgroundSent {
    size_bytes: u64,
    padding: bool,
}

/// Sends one background datagram as `sequence`: the recording's next chunk when there is one
/// to send and it can be read, else padding. `None` if the send failed.
fn send_background(
    io: &mut impl SenderIo,
    link: LinkHandle,
    receiver: SocketAddrV4,
    header: Header,
    sequence: u32,
    now_us: u64,
    recording: Option<&mut RecordingUpload>,
) -> Option<BackgroundSent> {
    match recording.map(|upload| send_chunk(io, link, receiver, header, sequence, now_us, upload)) {
        Some(ChunkSent::Sent(size_bytes)) => {
            return Some(BackgroundSent {
                size_bytes,
                padding: false,
            });
        }
        Some(ChunkSent::Failed) => return None,
        Some(ChunkSent::NothingToSend) | None => {}
    }
    let packet = Packet::Data(Data {
        header,
        class: TrafficClass::Background,
        sequence,
        sent_at_us: wrapped_us(now_us),
        payload: &PADDING,
    });
    send_packet(io, link, receiver, &packet).then_some(BackgroundSent {
        size_bytes: PADDING_DATAGRAM_BYTES,
        padding: true,
    })
}

/// The queue the upload aims under on `link`: the queue target while the live stream runs;
/// once it is idle (no live packet waits), also what the link's jitter and the time to send the
/// packets one ACK covers read as a queue where there is none, so slow or jittery links fill.
fn spare_queue_target_us(link: &Link, now_us: u64, queue_target_us: u64, live_idle: bool) -> u64 {
    if !live_idle {
        return queue_target_us;
    }
    let ack_burst_bits = u64::from(ACK_EVERY_PACKETS) * MAX_DATAGRAM_LEN as u64 * BITS_PER_BYTE;
    let serialisation_us =
        ack_burst_bits * MICROSECONDS_PER_SECOND / link.capacity_bps(now_us).max(1);
    queue_target_us + serialisation_us + JITTER_ALLOWANCE * link.estimate.rtt_variation_us()
}

/// Most of the recording a link may have in flight on spare room: the share of what the live
/// traffic leaves of its congestion window, at the capacity it is estimated to carry. The
/// window is twice the bandwidth-delay product, so the upload can find more than the estimate
/// (which only says what the link was given); the spare window keeps the queue short.
fn spare_cap_bytes(link: &Link, now_us: u64, share_percent: u64) -> u64 {
    let window_bytes = link
        .estimate
        .congestion_window_bytes(link.capacity_bps(now_us));
    window_bytes.saturating_sub(link.in_flight.live_view_bytes()) * share_percent / PERCENT
}

fn stall(
    link: &mut Link,
    now_us: u64,
    io: &mut impl SenderIo,
    event: SenderEvent,
    recording: Option<&mut RecordingUpload>,
) {
    if let Some(upload) = recording {
        upload.on_link_lost(link.id);
    }
    link.probe.on_stall(link.capacity_bps(now_us));
    link.spare_window.reset();
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
