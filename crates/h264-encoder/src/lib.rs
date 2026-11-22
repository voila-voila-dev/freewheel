//! An H.264 encoder whose bitrate can change while it runs, for the lab (M1b step 5): libx264
//! through a small C shim (`shim/x264_shim.c`). libx264 is GPL-2.0: this crate is a lab tool,
//! never linked into anything on the box, which encodes with Rockchip MPP
//! (`docs/technical/decisions.md`).
//!
//! Without libx264 at build time, [`Encoder::open`] always fails with
//! [`EncoderError::Unavailable`].

mod encoder;
mod encoder_error;
mod encoder_settings;

pub use encoder::Encoder;
pub use encoder_error::EncoderError;
pub use encoder_settings::EncoderSettings;
