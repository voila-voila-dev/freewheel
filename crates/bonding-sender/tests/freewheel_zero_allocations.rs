//! The freewheel per-packet path allocates nothing, on either side, capacity reports for the
//! encoder included, and with capacity probing and the recording upload on: the count does
//! not grow with traffic.

use std::net::{Ipv4Addr, SocketAddrV4};

use alloc_counter::{CountingAllocator, count_allocations};
use bonding_protocol::freewheel::MAX_DATAGRAM_LEN;
use std::path::PathBuf;

use bonding_receiver::freewheel::{
    GroupHandle, IoFailure as ReceiverIoFailure, Receiver, ReceiverIo,
    RecordingSettings as ReceiverRecordingSettings,
};
use bonding_sender::freewheel::{
    IoFailure, LinkHandle, ProbeTarget, RecordingSettings, Sender, SenderIo,
};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const RECEIVER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 200, 0, 1), 5_000);
const CALLER: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 7_001);
const LINKS: u8 = 4;
const QUEUE_LEN: usize = 8;

/// A few datagrams in fixed buffers: what one side sent, for the other to read.
struct Mailbox {
    datagrams: [[u8; MAX_DATAGRAM_LEN]; QUEUE_LEN],
    lens: [usize; QUEUE_LEN],
    links: [u64; QUEUE_LEN],
    count: usize,
    srt_bytes: usize,
    /// The box's recording: this many bytes so far, each one its offset's low byte.
    recording_bytes: u64,
    /// Bytes of the recording the server took.
    copied_bytes: usize,
}

impl Mailbox {
    const fn new() -> Self {
        Self {
            datagrams: [[0; MAX_DATAGRAM_LEN]; QUEUE_LEN],
            lens: [0; QUEUE_LEN],
            links: [0; QUEUE_LEN],
            count: 0,
            srt_bytes: 0,
            recording_bytes: 0,
            copied_bytes: 0,
        }
    }

    fn put(&mut self, link: u64, datagram: &[u8]) {
        if self.count < QUEUE_LEN {
            self.datagrams[self.count][..datagram.len()].copy_from_slice(datagram);
            self.lens[self.count] = datagram.len();
            self.links[self.count] = link;
            self.count += 1;
        }
    }
}

impl SenderIo for Mailbox {
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
        self.put(link.get(), datagram);
        Ok(())
    }
    fn send_to_srt(&mut self, _: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.srt_bytes += datagram.len();
        Ok(())
    }
    fn recording_bytes(&mut self) -> Option<u64> {
        Some(self.recording_bytes)
    }
    fn read_recording(&mut self, offset: u64, out: &mut [u8]) -> Result<(), IoFailure> {
        for (at, byte) in (offset..).zip(out.iter_mut()) {
            *byte = at.to_le_bytes()[0];
        }
        Ok(())
    }
}

impl ReceiverIo for Mailbox {
    fn send_to_link(
        &mut self,
        link: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), ReceiverIoFailure> {
        self.put(u64::from(link.ip().octets()[1]) - 1, datagram);
        Ok(())
    }
    fn open_srt(&mut self, _: GroupHandle) -> Result<(), ReceiverIoFailure> {
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, datagram: &[u8]) -> Result<(), ReceiverIoFailure> {
        self.srt_bytes += datagram.len();
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), ReceiverIoFailure> {
        bytes.fill(1);
        Ok(())
    }
    fn write_recording(&mut self, _: u64, _: u64, bytes: &[u8]) -> Result<(), ReceiverIoFailure> {
        self.copied_bytes += bytes.len();
        Ok(())
    }
}

fn link_address(link: u64) -> SocketAddrV4 {
    SocketAddrV4::new(
        Ipv4Addr::new(10, u8::try_from(link + 1).unwrap_or(0), 0, 2),
        40_000,
    )
}

/// Hands everything in flight to the other side until both are quiet.
fn exchange(
    now_us: u64,
    sender: &mut Sender,
    to_receiver: &mut Mailbox,
    receiver: &mut Receiver,
    to_sender: &mut Mailbox,
) {
    while to_receiver.count > 0 || to_sender.count > 0 {
        for index in 0..std::mem::take(&mut to_receiver.count) {
            let datagram = to_receiver.datagrams[index];
            let from = link_address(to_receiver.links[index]);
            receiver.handle_link_datagram(
                now_us,
                from,
                &datagram[..to_receiver.lens[index]],
                to_sender,
            );
        }
        for index in 0..std::mem::take(&mut to_sender.count) {
            let datagram = to_sender.datagrams[index];
            let link = handle_of(sender, to_sender.links[index]);
            if let Some(link) = link {
                sender.handle_link_datagram(
                    now_us,
                    link,
                    &datagram[..to_sender.lens[index]],
                    to_receiver,
                );
            }
        }
    }
}

fn handle_of(sender: &Sender, link: u64) -> Option<LinkHandle> {
    sender
        .links()
        .map(|status| status.handle)
        .find(|handle| handle.get() == link)
}

/// Streams `packets` SRT packets; with `recording`, probing runs and carries a recording that
/// grows 1 kB per packet. Returns the SRT bytes delivered both ways, and the recording bytes.
fn stream(packets: u32, recording: bool) -> (usize, usize) {
    let mut to_receiver = Mailbox::new();
    let mut to_sender = Mailbox::new();
    let mut sender = Sender::new(42, RECEIVER);
    let mut receiver = Receiver::new();
    if recording {
        sender.enable_probing(ProbeTarget {
            stream_max_wire_bps: 50_000_000,
            wanted_capacity_bps: 1_000_000_000,
        });
        sender.enable_recording(&RecordingSettings::new(PathBuf::from("/unused")));
        receiver.enable_recording(&ReceiverRecordingSettings::new(PathBuf::from("/unused")));
    }
    let report_every = if recording { 30 } else { 300 };
    let sources: Vec<Ipv4Addr> = (1..=LINKS).map(|k| Ipv4Addr::new(10, k, 0, 2)).collect();
    sender.reload_sources(&sources, &mut to_receiver);
    sender.open_links(&mut to_receiver);
    sender.housekeeping(0, &mut to_receiver);
    exchange(
        0,
        &mut sender,
        &mut to_receiver,
        &mut receiver,
        &mut to_sender,
    );
    let mut packet = [0u8; 1_316];
    for number in 0..packets {
        let now_us = u64::from(number) * 300;
        packet[..4].copy_from_slice(&number.to_be_bytes());
        sender.handle_srt_datagram(now_us, CALLER, &packet, &mut to_receiver);
        exchange(
            now_us,
            &mut sender,
            &mut to_receiver,
            &mut receiver,
            &mut to_sender,
        );
        sender.housekeeping(now_us, &mut to_receiver);
        receiver.housekeeping(now_us, &mut to_sender);
        if recording {
            to_receiver.recording_bytes += 1_000;
        }
        if number % report_every == 0 {
            let mut report = [0u8; bitrate_control::MAX_REPORT_LEN];
            let _ = sender.capacity_report(now_us).encode(&mut report);
        }
        exchange(
            now_us,
            &mut sender,
            &mut to_receiver,
            &mut receiver,
            &mut to_sender,
        );
    }
    (
        to_sender.srt_bytes + to_receiver.srt_bytes,
        to_sender.copied_bytes,
    )
}

#[test]
fn freewheel_allocates_nothing_per_packet() {
    let ((small_bytes, _), small) = count_allocations(|| stream(1_000, false));
    let ((large_bytes, _), large) = count_allocations(|| stream(100_000, false));
    assert!(small_bytes > 0 && large_bytes > small_bytes);
    assert_eq!(
        small, large,
        "{small} allocations for 1k packets, {large} for 100k"
    );
}

#[test]
fn the_recording_upload_allocates_nothing_per_chunk() {
    let ((_, small_copied), small) = count_allocations(|| stream(1_000, true));
    let ((_, large_copied), large) = count_allocations(|| stream(100_000, true));
    assert!(
        small_copied > 0 && large_copied > small_copied,
        "the recording went: {small_copied}, {large_copied} bytes"
    );
    assert_eq!(
        small, large,
        "{small} allocations for 1k packets, {large} for 100k"
    );
}
