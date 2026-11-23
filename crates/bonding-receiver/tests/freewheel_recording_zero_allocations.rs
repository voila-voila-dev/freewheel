//! Keeping the recording allocates nothing per chunk on the receiver's thread: the state
//! machine, and the hand-off to the writing thread.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;

use alloc_counter::{CountingAllocator, count_allocations};
use bonding_protocol::freewheel::{
    BackgroundPayload, Data, Header, MAX_DATA_PAYLOAD_LEN, MAX_DATAGRAM_LEN,
    MAX_RECORDING_CHUNK_LEN, Packet, RecordingChunk, TrafficClass,
};
use bonding_receiver::freewheel::{
    GroupHandle, IoFailure, Receiver, ReceiverIo, RecordingSettings,
};
use bonding_receiver::server::RecordingWriter;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const LINK: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(10, 1, 0, 2), 40_000);
const HEADER: Header = Header {
    session_id: 9,
    link_id: 0,
};

/// Counts what goes out, keeps nothing.
#[derive(Default)]
struct CountingIo {
    datagrams: usize,
    recording_bytes: usize,
}

impl ReceiverIo for CountingIo {
    fn send_to_link(&mut self, _: SocketAddrV4, _: &[u8]) -> Result<(), IoFailure> {
        self.datagrams += 1;
        Ok(())
    }
    fn open_srt(&mut self, _: GroupHandle) -> Result<(), IoFailure> {
        Ok(())
    }
    fn send_to_srt(&mut self, _: GroupHandle, _: &[u8]) -> Result<(), IoFailure> {
        Ok(())
    }
    fn close_srt(&mut self, _: GroupHandle) {}
    fn fill_random(&mut self, _: &mut [u8]) -> Result<(), IoFailure> {
        Ok(())
    }
    fn write_recording(&mut self, _: u64, _: u64, bytes: &[u8]) -> Result<(), IoFailure> {
        self.recording_bytes += bytes.len();
        Ok(())
    }
}

/// Encodes chunk `index` (a full one) into `out`.
fn chunk_packet(index: u32, out: &mut [u8; MAX_DATAGRAM_LEN]) -> usize {
    let bytes = [0x3C; MAX_RECORDING_CHUNK_LEN];
    let mut payload = [0u8; MAX_DATA_PAYLOAD_LEN];
    let len = BackgroundPayload::Chunk(RecordingChunk {
        offset: u64::from(index) * MAX_RECORDING_CHUNK_LEN as u64,
        bytes: &bytes,
    })
    .encode(&mut payload)
    .unwrap_or(0);
    Packet::Data(Data {
        header: HEADER,
        class: TrafficClass::Background,
        sequence: index,
        sent_at_us: index,
        payload: payload.get(..len).unwrap_or_default(),
    })
    .encode(out)
    .unwrap_or(0)
}

fn upload(receiver: &mut Receiver, io: &mut CountingIo, chunks: u32) {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    for index in 0..chunks {
        let len = chunk_packet(index, &mut out);
        receiver.handle_link_datagram(u64::from(index), LINK, &out[..len], io);
        receiver.housekeeping(u64::from(index), io);
    }
}

#[test]
fn keeping_chunks_allocates_nothing_per_chunk() {
    let mut receiver = Receiver::new();
    receiver.enable_recording(&RecordingSettings::new(PathBuf::from("/unused")));
    let mut io = CountingIo::default();
    let mut join = [0u8; MAX_DATAGRAM_LEN];
    let len = Packet::Join(HEADER).encode(&mut join).unwrap_or(0);
    receiver.handle_link_datagram(0, LINK, &join[..len], &mut io);
    let ((), small) = count_allocations(|| upload(&mut receiver, &mut io, 100));
    let ((), large) = count_allocations(|| upload(&mut receiver, &mut io, 10_000));
    assert_eq!(
        small, large,
        "{small} allocations for 100 chunks, {large} for 10 000"
    );
    assert_eq!(io.recording_bytes, 10_100 * MAX_RECORDING_CHUNK_LEN);
}

#[test]
fn handing_chunks_to_the_writer_allocates_nothing() {
    let directory =
        std::env::temp_dir().join(format!("freewheel-recording-alloc-{}", std::process::id()));
    let Ok(writer) = RecordingWriter::start(&RecordingSettings::new(directory.clone())) else {
        unreachable!("a temporary directory can be created")
    };
    let bytes = [0x5Au8; MAX_RECORDING_CHUNK_LEN];
    let write = |chunks: u64| {
        for index in 0..chunks {
            let _ = writer.write(1, index * MAX_RECORDING_CHUNK_LEN as u64, &bytes);
        }
    };
    let ((), small) = count_allocations(|| write(10));
    let ((), large) = count_allocations(|| write(1_000));
    writer.finish();
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(
        small, large,
        "{small} allocations for 10 chunks, {large} for 1 000"
    );
}
