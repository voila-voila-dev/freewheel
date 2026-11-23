//! The freewheel receiver keeps the recording a session uploads (M1b step 6): each chunk at its
//! offset, whatever the order and however many copies, acknowledged by sequence once taken; and
//! without a recording, background data is dropped as before.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;

use bonding_protocol::freewheel::{
    BackgroundPayload, Data, Header, MAX_DATA_PAYLOAD_LEN, MAX_DATAGRAM_LEN, Packet,
    RecordingChunk, SequenceRange, TrafficClass,
};
use bonding_receiver::freewheel::{
    ACK_EVERY_PACKETS, GroupHandle, IoFailure, Receiver, ReceiverIo, RecordingSettings,
};
use bonding_receiver::server::{RecordingWriter, recording_copy_path};

/// A receiver's outside world with the recording in memory.
#[derive(Default)]
struct MemoryIo {
    to_links: Vec<Vec<u8>>,
    to_srt: Vec<Vec<u8>>,
    copy: Vec<u8>,
    writes: usize,
    /// Writes refused from now on.
    full: bool,
}

impl ReceiverIo for MemoryIo {
    fn send_to_link(&mut self, _: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        self.to_links.push(datagram.to_vec());
        Ok(())
    }
    fn open_srt(&mut self, _: GroupHandle) -> Result<(), IoFailure> {
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure> {
        self.to_srt.push(datagram.to_vec());
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure> {
        bytes.fill(1);
        Ok(())
    }
    fn write_recording(
        &mut self,
        session_id: u64,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), IoFailure> {
        assert_eq!(session_id, HEADER.session_id);
        if self.full {
            return Err(IoFailure);
        }
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        let end = start + bytes.len();
        if self.copy.len() < end {
            self.copy.resize(end, 0);
        }
        self.copy[start..end].copy_from_slice(bytes);
        self.writes += 1;
        Ok(())
    }
}

impl MemoryIo {
    /// The ranges of every ACK sent so far, one list per ACK, and forgets them.
    fn take_ack_ranges(&mut self) -> Vec<Vec<SequenceRange>> {
        std::mem::take(&mut self.to_links)
            .iter()
            .filter_map(|datagram| match Packet::decode(datagram) {
                Ok(Packet::Ack(ack)) => Some(ack.ranges.iter().collect()),
                _ => None,
            })
            .collect()
    }
}

const LINK: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 1, 0, 2), 40_000);
const HEADER: Header = Header {
    session_id: 0xC0FF_EE00,
    link_id: 0,
};

fn packet(sequence: u32, class: TrafficClass, payload: &[u8]) -> Vec<u8> {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let len = Packet::Data(Data {
        header: HEADER,
        class,
        sequence,
        sent_at_us: sequence,
        payload,
    })
    .encode(&mut out)
    .unwrap_or(0);
    out[..len].to_vec()
}

fn chunk(sequence: u32, offset: u64, bytes: &[u8]) -> Vec<u8> {
    let mut payload = [0u8; MAX_DATA_PAYLOAD_LEN];
    let len = BackgroundPayload::Chunk(RecordingChunk { offset, bytes })
        .encode(&mut payload)
        .unwrap_or(0);
    packet(sequence, TrafficClass::Background, &payload[..len])
}

fn padding(sequence: u32) -> Vec<u8> {
    packet(
        sequence,
        TrafficClass::Background,
        &[0; MAX_DATA_PAYLOAD_LEN],
    )
}

fn joined(recording: bool) -> (Receiver, MemoryIo) {
    let mut receiver = Receiver::new();
    if recording {
        receiver.enable_recording(&RecordingSettings::new(PathBuf::from("/unused")));
    }
    let mut io = MemoryIo::default();
    let mut join = [0u8; MAX_DATAGRAM_LEN];
    let len = Packet::Join(HEADER).encode(&mut join).unwrap_or(0);
    receiver.handle_link_datagram(0, LINK, &join[..len], &mut io);
    io.to_links.clear();
    (receiver, io)
}

/// 10 000 bytes of a recording that never repeats a byte pattern.
fn recording() -> Vec<u8> {
    (0..10_000_u32)
        .map(|index| ((index * 7 + index / 251) % 256).to_le_bytes()[0])
        .collect()
}

#[test]
fn chunks_out_of_order_and_repeated_rebuild_the_recording() {
    let (mut receiver, mut io) = joined(true);
    let recording = recording();
    let pieces: Vec<(u64, &[u8])> = recording
        .chunks(1_000)
        .enumerate()
        .map(|(index, bytes)| (index as u64 * 1_000, bytes))
        .collect();
    // Odd pieces first, then even, then every piece again.
    let order = (1..10).step_by(2).chain((0..10).step_by(2)).chain(0..10);
    for (sequence, index) in (0..).zip(order) {
        let (offset, bytes) = pieces[index];
        receiver.handle_link_datagram(1, LINK, &chunk(sequence, offset, bytes), &mut io);
    }
    assert_eq!(io.copy, recording);
    assert_eq!(io.writes, 20);
    assert!(io.to_srt.is_empty(), "the recording never reaches SRT");
}

#[test]
fn every_ack_carries_the_latest_chunks_taken() {
    let (mut receiver, mut io) = joined(true);
    for sequence in 0..ACK_EVERY_PACKETS {
        receiver.handle_link_datagram(1, LINK, &chunk(sequence, 0, &[1]), &mut io);
    }
    assert_eq!(
        io.take_ack_ranges(),
        [vec![SequenceRange {
            first: 0,
            last: ACK_EVERY_PACKETS - 1
        }]],
        "one run, the chunk that triggered the ACK included"
    );
    // A live packet in between splits the runs; padding is acknowledged by link only.
    receiver.handle_link_datagram(
        2,
        LINK,
        &packet(10, TrafficClass::Live, &[0x47; 188]),
        &mut io,
    );
    receiver.handle_link_datagram(2, LINK, &chunk(11, 0, &[1]), &mut io);
    receiver.handle_link_datagram(2, LINK, &padding(12), &mut io);
    receiver.handle_link_datagram(2, LINK, &chunk(13, 0, &[1]), &mut io);
    assert_eq!(
        io.take_ack_ranges(),
        [vec![
            SequenceRange {
                first: 0,
                last: ACK_EVERY_PACKETS - 1
            },
            SequenceRange {
                first: 11,
                last: 11
            },
            SequenceRange {
                first: 13,
                last: 13
            },
        ]]
    );
}

#[test]
fn an_ack_keeps_only_the_latest_ranges() {
    let (mut receiver, mut io) = joined(true);
    let limit = RecordingSettings::new(PathBuf::new()).acknowledged_ranges;
    let chunks = u32::try_from(limit).unwrap_or(0) + 4;
    for index in 0..chunks {
        receiver.handle_link_datagram(1, LINK, &chunk(index * 2, 0, &[1]), &mut io);
    }
    let acks = io.take_ack_ranges();
    let last = acks.last().cloned().unwrap_or_default();
    assert_eq!(last.len(), limit);
    assert_eq!(
        last.last().copied(),
        Some(SequenceRange {
            first: (chunks - 1) * 2,
            last: (chunks - 1) * 2
        })
    );
}

#[test]
fn a_chunk_the_disk_cannot_take_is_not_acknowledged() {
    let (mut receiver, mut io) = joined(true);
    io.full = true;
    for sequence in 0..ACK_EVERY_PACKETS {
        receiver.handle_link_datagram(1, LINK, &chunk(sequence, 0, &[1]), &mut io);
    }
    receiver.handle_link_datagram(
        1,
        LINK,
        &packet(9, TrafficClass::Live, &[0x47; 188]),
        &mut io,
    );
    assert_eq!(io.take_ack_ranges(), [Vec::<SequenceRange>::new()]);
    assert_eq!(io.to_srt.len(), 1, "the live stream goes on");
}

#[test]
fn without_a_recording_background_data_is_dropped_as_before() {
    let (mut receiver, mut io) = joined(false);
    for sequence in 0..ACK_EVERY_PACKETS * 2 {
        receiver.handle_link_datagram(1, LINK, &chunk(sequence, 0, &[1, 2, 3]), &mut io);
    }
    assert_eq!(io.writes, 0);
    assert_eq!(
        io.take_ack_ranges(),
        [Vec::<SequenceRange>::new(), Vec::new()],
        "acknowledged by link, never by range"
    );
}

#[test]
fn malformed_background_data_is_counted_and_nothing_else() {
    let (mut receiver, mut io) = joined(true);
    for (sequence, payload) in [[0x02, 0, 0], [0x00, 1, 0], [0x01, 0, 0], [0x00, 0, 0]]
        .iter()
        .enumerate()
    {
        let sequence = u32::try_from(sequence).unwrap_or(0);
        receiver.handle_link_datagram(
            1,
            LINK,
            &packet(sequence, TrafficClass::Background, payload),
            &mut io,
        );
    }
    assert_eq!(io.writes, 0);
    assert_eq!(io.take_ack_ranges(), [Vec::<SequenceRange>::new()]);
}

#[test]
fn the_writer_puts_each_chunk_at_its_offset_on_disk() {
    let directory =
        std::env::temp_dir().join(format!("freewheel-recording-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let settings = RecordingSettings::new(directory.clone());
    let Ok(writer) = RecordingWriter::start(&settings) else {
        unreachable!("a temporary directory can be created")
    };
    let recording = recording();
    let pieces: Vec<(usize, &[u8])> = recording.chunks(1_471).enumerate().collect();
    for (index, bytes) in pieces.iter().rev().chain(pieces.iter()) {
        assert_eq!(
            writer.write(HEADER.session_id, (*index * 1_471) as u64, bytes),
            Ok(())
        );
    }
    writer.finish();
    let copy =
        std::fs::read(recording_copy_path(&directory, HEADER.session_id)).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(copy, recording);
}

#[test]
fn a_full_writer_refuses_at_once() {
    let directory =
        std::env::temp_dir().join(format!("freewheel-recording-full-{}", std::process::id()));
    let settings = RecordingSettings {
        writer_queue_chunks: 1,
        ..RecordingSettings::new(directory.clone())
    };
    let Ok(writer) = RecordingWriter::start(&settings) else {
        unreachable!("a temporary directory can be created")
    };
    // The thread drains the queue meanwhile: some writes may be taken, but never all of
    // 10 000 at once into a queue of one, and none blocks.
    let refused = (0..10_000_u64)
        .filter(|offset| writer.write(1, *offset, &[1]).is_err())
        .count();
    writer.finish();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(refused > 0);
}
