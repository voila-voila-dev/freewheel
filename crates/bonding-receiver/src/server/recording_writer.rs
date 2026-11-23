//! Writes the recordings the senders upload to disk, on a thread of its own, so the disk never
//! holds up the live stream.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::thread::{self, JoinHandle};

use bonding_protocol::freewheel::MAX_RECORDING_CHUNK_LEN;

use crate::freewheel::RecordingSettings;
use crate::srtla::IoFailure;

/// The copy of a session's recording: `<directory>/<session id, 16 hex digits>.recording`.
#[must_use]
pub fn recording_copy_path(directory: &Path, session_id: u64) -> PathBuf {
    directory.join(format!("{session_id:016x}.recording"))
}

/// One chunk on its way to the disk, copied out of the datagram it came in.
struct WriteRequest {
    session_id: u64,
    offset: u64,
    len: usize,
    bytes: [u8; MAX_RECORDING_CHUNK_LEN],
}

/// A bounded queue of chunks and the thread that writes them, each at its offset in its
/// session's copy ([`recording_copy_path`]). The queue is allocated once: taking a chunk
/// allocates nothing.
#[derive(Debug)]
pub struct RecordingWriter {
    queue: SyncSender<WriteRequest>,
    thread: JoinHandle<()>,
}

impl RecordingWriter {
    /// Creates `settings.directory` if needed and starts the writing thread, with room for
    /// `settings.writer_queue_chunks` chunks.
    ///
    /// # Errors
    /// The directory cannot be created, or the thread cannot start.
    pub fn start(settings: &RecordingSettings) -> io::Result<Self> {
        std::fs::create_dir_all(&settings.directory)?;
        let (queue, requests) = sync_channel(settings.writer_queue_chunks);
        let directory = settings.directory.clone();
        let thread = thread::Builder::new()
            .name("recording-writer".to_owned())
            .spawn(move || write_until_closed(&directory, &requests))?;
        Ok(Self { queue, thread })
    }

    /// Writes what is queued, then stops the thread.
    pub fn finish(self) {
        drop(self.queue);
        let _ = self.thread.join();
    }

    /// Queues `bytes` for `offset` in the session's copy; fails at once, without waiting, when
    /// the queue is full or the thread has stopped (the sender sends the bytes again).
    ///
    /// # Errors
    /// The bytes were not queued.
    pub fn write(&self, session_id: u64, offset: u64, bytes: &[u8]) -> Result<(), IoFailure> {
        let mut request = WriteRequest {
            session_id,
            offset,
            len: bytes.len(),
            bytes: [0; MAX_RECORDING_CHUNK_LEN],
        };
        request
            .bytes
            .get_mut(..bytes.len())
            .ok_or(IoFailure)?
            .copy_from_slice(bytes);
        self.queue.try_send(request).map_err(|error| match error {
            TrySendError::Full(_) | TrySendError::Disconnected(_) => IoFailure,
        })
    }
}

/// The writing thread: each chunk at its offset, until the queue's sender is dropped. A failed
/// write is logged; the copy is then incomplete, which the comparison with the recording shows.
fn write_until_closed(directory: &Path, requests: &Receiver<WriteRequest>) {
    let mut copies: HashMap<u64, File> = HashMap::new();
    for request in requests {
        let path = recording_copy_path(directory, request.session_id);
        let copy = match copies.entry(request.session_id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Never truncated: what an earlier run of the receiver wrote stays where it is.
                let opened = OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(&path);
                match opened {
                    Ok(file) => {
                        eprintln!("recording: writing {}", path.display());
                        entry.insert(file)
                    }
                    Err(error) => {
                        eprintln!("recording: cannot open {}: {error}", path.display());
                        continue;
                    }
                }
            }
        };
        let bytes = request.bytes.get(..request.len).unwrap_or_default();
        if let Err(error) = copy.write_all_at(bytes, request.offset) {
            eprintln!(
                "recording: cannot write {} bytes at {} in {}: {error}",
                bytes.len(),
                request.offset,
                path.display()
            );
        }
    }
}
