//! What the encoder is opened with.

/// Picture size, frame rate, keyframe interval and the starting bitrate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderSettings {
    /// Width, in pixels (even).
    pub width: u32,
    /// Height, in pixels (even).
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// A keyframe every this many frames.
    pub keyint: u32,
    /// Starting bitrate, in bit/s.
    pub bitrate_bps: u32,
}

impl EncoderSettings {
    /// Bytes of one I420 frame: a full-size luma plane and two quarter-size chroma planes.
    #[must_use]
    pub const fn frame_bytes(&self) -> usize {
        let luma = self.width as usize * self.height as usize;
        luma + luma / 2
    }
}
