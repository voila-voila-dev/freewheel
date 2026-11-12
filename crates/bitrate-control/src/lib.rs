//! Adaptive bitrate controller, specified in `docs/technical/protocol/bitrate-control.md`.
//!
//! Pure: it takes link statistics and a time, returns the next bitrate. No I/O.
#![no_std]

mod bounds;

pub use bounds::Bounds;
