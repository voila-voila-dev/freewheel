//! A simulated network for the freewheel sender and receiver: links with a delay, a rate,
//! a queue, and an on/off switch, in simulated microseconds. Deterministic.

#![allow(dead_code)]

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::net::{Ipv4Addr, SocketAddrV4};

use bitrate_control::CapacityReport;
use bonding_receiver::freewheel::{
    GroupHandle, IoFailure as ReceiverIoFailure, Receiver, ReceiverIo,
};
use bonding_sender::freewheel::{IoFailure, LinkHandle, ProbeTarget, Sender, SenderIo};

/// Where the receiver listens.
pub const RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), 5_000);
const CALLER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7_001);
/// Payload of one SRT data packet in the lab.
pub const SRT_PACKET_BYTES: usize = 1_316;
const TICK_US: u64 = 1_000;
/// Capacity reports, once probing is on: as often as `freewheel-sender` sends them.
const REPORT_PERIOD_US: u64 = 100_000;
const BITS_PER_BYTE: u64 = 8;
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;

/// One link's behaviour, uplink (box → server); the downlink only has the delay.
#[derive(Debug, Clone, Copy)]
pub struct LinkSpec {
    /// One-way delay, in microseconds.
    pub delay_us: u64,
    /// Uplink capacity, in bit/s.
    pub rate_bps: u64,
    /// Longest queue before tail drop, in microseconds of sending.
    pub max_queue_us: u64,
}

impl LinkSpec {
    /// A link of `rate_bps` with `delay_us` one way and a 200 ms queue.
    pub const fn new(delay_us: u64, rate_bps: u64) -> Self {
        Self {
            delay_us,
            rate_bps,
            max_queue_us: 200_000,
        }
    }
}

struct SimLink {
    spec: LinkSpec,
    up: bool,
    busy_until_us: u64,
    data_packets: u64,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Delivery {
    ToReceiver { link: usize, datagram: Vec<u8> },
    ToSender { link: usize, datagram: Vec<u8> },
}

#[derive(Default)]
struct SenderSide {
    outbox: Vec<(LinkHandle, Vec<u8>)>,
    to_caller: u64,
}

impl SenderIo for SenderSide {
    fn open_link(&mut self, _: LinkHandle, _: Ipv4Addr) -> Result<(), IoFailure> {
        Ok(())
    }
    fn close_link(&mut self, _: LinkHandle) {}
    fn send_on_link(
        &mut self,
        link: LinkHandle,
        _: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), IoFailure> {
        self.outbox.push((link, datagram.to_vec()));
        Ok(())
    }
    fn send_to_srt(&mut self, _: SocketAddrV4, _: &[u8]) -> Result<(), IoFailure> {
        self.to_caller += 1;
        Ok(())
    }
}

#[derive(Default)]
struct ReceiverSide {
    outbox: Vec<(SocketAddrV4, Vec<u8>)>,
    delivered: Vec<u32>,
    latencies_us: Vec<u64>,
    now_us: u64,
    sent_at_us: Vec<u64>,
    group: Option<GroupHandle>,
}

impl ReceiverIo for ReceiverSide {
    fn send_to_link(
        &mut self,
        link: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), ReceiverIoFailure> {
        self.outbox.push((link, datagram.to_vec()));
        Ok(())
    }
    fn open_srt(&mut self, group: GroupHandle) -> Result<(), ReceiverIoFailure> {
        self.group = Some(group);
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, datagram: &[u8]) -> Result<(), ReceiverIoFailure> {
        if let Some(word) = datagram.get(..4) {
            let mut number = [0u8; 4];
            number.copy_from_slice(word);
            let number = u32::from_be_bytes(number);
            self.delivered.push(number);
            let sent_at_us = usize::try_from(number)
                .ok()
                .and_then(|index| self.sent_at_us.get(index))
                .copied()
                .unwrap_or(self.now_us);
            self.latencies_us.push(self.now_us - sent_at_us);
        }
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), ReceiverIoFailure> {
        bytes.fill(7);
        Ok(())
    }
}

/// The box, the links and the server, in simulated time.
pub struct Network {
    now_us: u64,
    links: Vec<SimLink>,
    handles: Vec<LinkHandle>,
    pub sender: Sender,
    pub receiver: Receiver,
    sender_side: SenderSide,
    receiver_side: ReceiverSide,
    in_transit: BinaryHeap<Reverse<(u64, u64, Delivery)>>,
    next_order: u64,
    sent: u32,
    next_stream_us: u64,
    /// Capacity reports taken every [`REPORT_PERIOD_US`] once probing is on, latest last.
    reports: Option<Vec<CapacityReport>>,
    next_report_us: u64,
}

fn source(link: usize) -> Ipv4Addr {
    Ipv4Addr::new(10, u8::try_from(link + 1).unwrap_or(0), 0, 2)
}

fn source_address(link: usize) -> SocketAddrV4 {
    SocketAddrV4::new(source(link), 40_000)
}

impl Network {
    /// A sender with one link per spec, all up, and a receiver.
    pub fn new(specs: &[LinkSpec]) -> Self {
        let mut sender_side = SenderSide::default();
        let mut sender = Sender::new(0x5E55_1011, RECEIVER);
        let sources: Vec<Ipv4Addr> = (0..specs.len()).map(source).collect();
        sender.reload_sources(&sources, &mut sender_side);
        sender.open_links(&mut sender_side);
        let handles = sources
            .iter()
            .filter_map(|address| {
                sender
                    .links()
                    .find(|link| link.source == *address)
                    .map(|link| link.handle)
            })
            .collect();
        Self {
            now_us: 0,
            links: specs
                .iter()
                .map(|spec| SimLink {
                    spec: *spec,
                    up: true,
                    busy_until_us: 0,
                    data_packets: 0,
                })
                .collect(),
            handles,
            sender,
            receiver: Receiver::new(),
            sender_side,
            receiver_side: ReceiverSide::default(),
            in_transit: BinaryHeap::new(),
            next_order: 0,
            sent: 0,
            next_stream_us: 0,
            reports: None,
            next_report_us: 0,
        }
    }

    /// Turns the sender's capacity probing on, and takes a capacity report every 100 ms as
    /// `freewheel-sender` does (probing learns the live traffic from them).
    pub fn enable_probing(&mut self, target: ProbeTarget) {
        self.sender.enable_probing(target);
        self.reports = Some(Vec::new());
        self.next_report_us = self.now_us;
    }

    /// The latest capacity report once probing is on (all zero before).
    pub fn last_report(&self) -> CapacityReport {
        self.reports
            .as_ref()
            .and_then(|reports| reports.last().copied())
            .unwrap_or_default()
    }

    /// Runs for `duration_us`, the SRT caller sending `stream_bps` in 1316-byte packets.
    pub fn run(&mut self, duration_us: u64, stream_bps: u64) {
        let interval_us = if stream_bps == 0 {
            u64::MAX
        } else {
            SRT_PACKET_BYTES as u64 * BITS_PER_BYTE * MICROSECONDS_PER_SECOND / stream_bps
        };
        let end_us = self.now_us + duration_us;
        if self.next_stream_us < self.now_us
            || self.next_stream_us > self.now_us.saturating_add(interval_us)
        {
            self.next_stream_us = self.now_us;
        }
        while self.now_us < end_us {
            let tick_end_us = self.now_us + TICK_US;
            while let Some(Reverse((at_us, _, _))) = self.in_transit.peek() {
                if *at_us > tick_end_us {
                    break;
                }
                let Some(Reverse((at_us, _, delivery))) = self.in_transit.pop() else {
                    break;
                };
                self.deliver(at_us, delivery);
            }
            while stream_bps > 0 && self.next_stream_us < tick_end_us {
                let at_us = self.next_stream_us;
                let mut packet = vec![0u8; SRT_PACKET_BYTES];
                packet[..4].copy_from_slice(&self.sent.to_be_bytes());
                self.sent += 1;
                self.receiver_side.sent_at_us.push(at_us);
                self.sender
                    .handle_srt_datagram(at_us, CALLER, &packet, &mut self.sender_side);
                self.flush(at_us);
                self.next_stream_us = self.next_stream_us.saturating_add(interval_us);
            }
            self.now_us = tick_end_us;
            if let Some(reports) = &mut self.reports
                && self.now_us >= self.next_report_us
            {
                reports.push(self.sender.capacity_report(self.now_us));
                self.next_report_us = self.now_us + REPORT_PERIOD_US;
            }
            self.sender.housekeeping(self.now_us, &mut self.sender_side);
            self.receiver
                .housekeeping(self.now_us, &mut self.receiver_side);
            self.flush(self.now_us);
        }
    }

    /// The simulated clock, in microseconds.
    pub const fn now_us(&self) -> u64 {
        self.now_us
    }

    /// Cuts or restores a link, both ways.
    pub fn set_up(&mut self, link: usize, up: bool) {
        if let Some(sim) = self.links.get_mut(link) {
            sim.up = up;
        }
    }

    /// Changes a link's uplink rate.
    pub fn set_rate(&mut self, link: usize, rate_bps: u64) {
        if let Some(sim) = self.links.get_mut(link) {
            sim.spec.rate_bps = rate_bps;
        }
    }

    /// Data packets each link carried so far (and resets the counts).
    pub fn take_data_counts(&mut self) -> Vec<u64> {
        self.links
            .iter_mut()
            .map(|link| std::mem::take(&mut link.data_packets))
            .collect()
    }

    /// SRT packets the caller sent so far.
    pub const fn sent(&self) -> u32 {
        self.sent
    }

    /// SRT packet numbers delivered to the listener, in arrival order.
    pub fn delivered(&self) -> &[u32] {
        &self.receiver_side.delivered
    }

    /// Latency of each delivered SRT packet (caller to listener), in microseconds, and resets
    /// the list.
    pub fn take_latencies_us(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.receiver_side.latencies_us)
    }

    /// Changes a link's queue, in microseconds of sending.
    pub fn set_max_queue_us(&mut self, link: usize, max_queue_us: u64) {
        if let Some(sim) = self.links.get_mut(link) {
            sim.spec.max_queue_us = max_queue_us;
        }
    }

    /// Datagrams the receiver sent back to the caller.
    pub const fn to_caller(&self) -> u64 {
        self.sender_side.to_caller
    }

    /// Whether the sender may use the link now.
    pub fn usable(&self, link: usize) -> bool {
        let handle = self.handles.get(link).copied();
        self.sender
            .links()
            .find(|status| Some(status.handle) == handle)
            .is_some_and(|status| status.usable)
    }

    /// The SRT listener answers: one datagram back towards the caller.
    pub fn srt_answer(&mut self) {
        if let Some(group) = self.receiver_side.group {
            self.receiver.handle_srt_datagram(
                self.now_us,
                group,
                &[0x80, 0x02, 0, 0],
                &mut self.receiver_side,
            );
            self.flush(self.now_us);
        }
    }

    fn deliver(&mut self, at_us: u64, delivery: Delivery) {
        match delivery {
            Delivery::ToReceiver { link, datagram } => {
                self.receiver_side.now_us = at_us;
                self.receiver.handle_link_datagram(
                    at_us,
                    source_address(link),
                    &datagram,
                    &mut self.receiver_side,
                );
            }
            Delivery::ToSender { link, datagram } => {
                if let Some(handle) = self.handles.get(link).copied() {
                    self.sender.handle_link_datagram(
                        at_us,
                        handle,
                        &datagram,
                        &mut self.sender_side,
                    );
                }
            }
        }
        self.flush(at_us);
    }

    /// Puts what both sides sent on the wire.
    fn flush(&mut self, now_us: u64) {
        for (handle, datagram) in std::mem::take(&mut self.sender_side.outbox) {
            let Some(link) = self.handles.iter().position(|h| *h == handle) else {
                continue;
            };
            let Some(sim) = self.links.get_mut(link) else {
                continue;
            };
            if !sim.up {
                continue;
            }
            let start_us = sim.busy_until_us.max(now_us);
            if start_us - now_us > sim.spec.max_queue_us {
                continue;
            }
            let serialisation_us = datagram.len() as u64 * BITS_PER_BYTE * MICROSECONDS_PER_SECOND
                / sim.spec.rate_bps.max(1);
            sim.busy_until_us = start_us + serialisation_us;
            if datagram.get(1) == Some(&0x01) {
                sim.data_packets += 1;
            }
            let at_us = sim.busy_until_us + sim.spec.delay_us;
            self.schedule(at_us, Delivery::ToReceiver { link, datagram });
        }
        for (address, datagram) in std::mem::take(&mut self.receiver_side.outbox) {
            let Some(link) = (0..self.links.len()).find(|k| source_address(*k) == address) else {
                continue;
            };
            let Some(sim) = self.links.get(link) else {
                continue;
            };
            if !sim.up {
                continue;
            }
            let at_us = now_us + sim.spec.delay_us;
            self.schedule(at_us, Delivery::ToSender { link, datagram });
        }
    }

    fn schedule(&mut self, at_us: u64, delivery: Delivery) {
        self.next_order += 1;
        self.in_transit
            .push(Reverse((at_us, self.next_order, delivery)));
    }
}
