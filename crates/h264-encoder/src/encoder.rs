//! The encoder itself, owning one x264 instance.

use crate::{EncoderError, EncoderSettings};

const BITS_PER_KILOBIT: u32 = 1_000;

/// One x264 encoder. Frames in, Annex B H.264 out; [`set_bitrate_bps`](Self::set_bitrate_bps)
/// at any time.
#[derive(Debug)]
pub struct Encoder {
    settings: EncoderSettings,
    bitrate_bps: u32,
    #[cfg(has_x264)]
    handle: core::ptr::NonNull<ffi::Handle>,
}

impl Encoder {
    /// Opens an encoder.
    ///
    /// # Errors
    /// [`EncoderError::InvalidSettings`], [`EncoderError::Failed`], or
    /// [`EncoderError::Unavailable`] when built without libx264.
    pub fn open(settings: EncoderSettings) -> Result<Self, EncoderError> {
        let valid = settings.width > 0
            && settings.height > 0
            && settings.width.is_multiple_of(2)
            && settings.height.is_multiple_of(2)
            && settings.fps > 0
            && settings.keyint > 0
            && settings.bitrate_bps >= BITS_PER_KILOBIT;
        if !valid {
            return Err(EncoderError::InvalidSettings);
        }
        Self::open_valid(settings)
    }

    #[cfg(has_x264)]
    fn open_valid(settings: EncoderSettings) -> Result<Self, EncoderError> {
        let int = |value: u32| i32::try_from(value).map_err(|_| EncoderError::InvalidSettings);
        // SAFETY: plain integers in; the shim returns an owned handle or null.
        let raw = unsafe {
            ffi::fw_x264_open(
                int(settings.width)?,
                int(settings.height)?,
                int(settings.fps)?,
                int(settings.bitrate_bps / BITS_PER_KILOBIT)?,
                int(settings.keyint)?,
            )
        };
        let handle = core::ptr::NonNull::new(raw).ok_or(EncoderError::Failed)?;
        Ok(Self {
            settings,
            bitrate_bps: settings.bitrate_bps,
            handle,
        })
    }

    #[cfg(not(has_x264))]
    #[allow(clippy::unnecessary_wraps)]
    const fn open_valid(_: EncoderSettings) -> Result<Self, EncoderError> {
        Err(EncoderError::Unavailable)
    }

    /// What it was opened with.
    #[must_use]
    pub const fn settings(&self) -> EncoderSettings {
        self.settings
    }

    /// The bitrate it encodes at now, in bit/s.
    #[must_use]
    pub const fn bitrate_bps(&self) -> u32 {
        self.bitrate_bps
    }

    /// Encodes the next frames at `bitrate_bps` (x264 counts in kbit/s).
    ///
    /// # Errors
    /// [`EncoderError::InvalidSettings`] under 1 kbit/s, [`EncoderError::Failed`] if x264
    /// refused it.
    pub fn set_bitrate_bps(&mut self, bitrate_bps: u32) -> Result<(), EncoderError> {
        let kbps = i32::try_from(bitrate_bps / BITS_PER_KILOBIT)
            .ok()
            .filter(|kbps| *kbps > 0)
            .ok_or(EncoderError::InvalidSettings)?;
        self.reconfigure(kbps)?;
        self.bitrate_bps = bitrate_bps;
        Ok(())
    }

    #[cfg(has_x264)]
    fn reconfigure(&mut self, kbps: i32) -> Result<(), EncoderError> {
        // SAFETY: the handle is live (owned by self, closed only in Drop).
        let status = unsafe { ffi::fw_x264_set_bitrate(self.handle.as_ptr(), kbps) };
        if status == 0 {
            Ok(())
        } else {
            Err(EncoderError::Failed)
        }
    }

    #[cfg(not(has_x264))]
    #[allow(clippy::unused_self, clippy::needless_pass_by_ref_mut)]
    const fn reconfigure(&mut self, _: i32) -> Result<(), EncoderError> {
        Err(EncoderError::Unavailable)
    }

    /// Encodes one I420 frame with presentation time `pts` (in frames), writes its H.264 into
    /// `out` and returns how many bytes that took (0 while x264 holds the frame back).
    ///
    /// # Errors
    /// [`EncoderError::WrongFrameSize`], [`EncoderError::OutputTooSmall`],
    /// [`EncoderError::Failed`].
    pub fn encode(
        &mut self,
        frame: &[u8],
        pts: i64,
        out: &mut [u8],
    ) -> Result<usize, EncoderError> {
        if frame.len() != self.settings.frame_bytes() {
            return Err(EncoderError::WrongFrameSize);
        }
        self.encode_frame(frame, pts, out)
    }

    #[cfg(has_x264)]
    fn encode_frame(
        &mut self,
        frame: &[u8],
        pts: i64,
        out: &mut [u8],
    ) -> Result<usize, EncoderError> {
        let capacity = i32::try_from(out.len()).unwrap_or(i32::MAX);
        // SAFETY: the handle is live; `frame` holds a whole I420 picture (checked above) that
        // x264 only reads during the call; `out` is writable for `capacity` bytes.
        let written = unsafe {
            ffi::fw_x264_encode(
                self.handle.as_ptr(),
                frame.as_ptr(),
                pts,
                out.as_mut_ptr(),
                capacity,
            )
        };
        match written {
            -2 => Err(EncoderError::OutputTooSmall),
            bytes => usize::try_from(bytes).map_err(|_| EncoderError::Failed),
        }
    }

    #[cfg(not(has_x264))]
    #[allow(clippy::unused_self, clippy::needless_pass_by_ref_mut)]
    const fn encode_frame(
        &mut self,
        _: &[u8],
        _: i64,
        _: &mut [u8],
    ) -> Result<usize, EncoderError> {
        Err(EncoderError::Unavailable)
    }
}

#[cfg(has_x264)]
impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: the handle came from fw_x264_open and is closed only here, once.
        unsafe { ffi::fw_x264_close(self.handle.as_ptr()) };
    }
}

#[cfg(has_x264)]
mod ffi {
    //! The shim's interface (`shim/x264_shim.c`).

    /// The shim's opaque encoder.
    #[repr(C)]
    pub struct Handle {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        pub fn fw_x264_open(
            width: i32,
            height: i32,
            fps: i32,
            bitrate_kbps: i32,
            keyint: i32,
        ) -> *mut Handle;
        pub fn fw_x264_set_bitrate(handle: *mut Handle, bitrate_kbps: i32) -> i32;
        pub fn fw_x264_encode(
            handle: *mut Handle,
            frame: *const u8,
            pts: i64,
            out: *mut u8,
            capacity: i32,
        ) -> i32;
        pub fn fw_x264_close(handle: *mut Handle);
    }
}
