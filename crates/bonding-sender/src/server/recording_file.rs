//! The recording to upload, read where the camera writes it.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use crate::srtla::IoFailure;

/// The recording file, opened once it exists; it may still be growing.
#[derive(Debug)]
pub struct RecordingFile {
    path: PathBuf,
    file: Option<File>,
}

impl RecordingFile {
    /// The recording at `path`, which may not exist yet.
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path, file: None }
    }

    /// Its length now, in bytes; `None` while it does not exist.
    pub fn len_bytes(&mut self) -> Option<u64> {
        if self.file.is_none() {
            self.file = File::open(&self.path).ok();
        }
        self.file
            .as_ref()
            .and_then(|file| file.metadata().ok())
            .map(|metadata| metadata.len())
    }

    /// Fills `out` with its bytes from `offset`.
    ///
    /// # Errors
    /// Not open yet, or shorter than asked.
    pub fn read_at(&self, offset: u64, out: &mut [u8]) -> Result<(), IoFailure> {
        self.file
            .as_ref()
            .ok_or(IoFailure)?
            .read_exact_at(out, offset)
            .map_err(|_| IoFailure)
    }
}
