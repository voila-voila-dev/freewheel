//! freewheel transport, receiving side (M1b, first candidate): gathers each session's links
//! and hands the SRT stream they carry to the SRT listener
//! (`docs/technical/protocol/freewheel.md`, "First candidate").
//!
//! [`Receiver`] decides; it reuses the SRTLA mode's [`ReceiverIo`] port (sockets, logs), a
//! session standing where SRTLA has a group. Nothing allocates per packet.

mod limits;
mod link;
mod receiver;
mod recording_settings;
mod session;
mod setting_error;

pub use limits::{
    ACK_EVERY_PACKETS, ACK_MAX_DELAY_US, LINK_TIMEOUT_US, MAX_LINKS_PER_SESSION, MAX_SESSIONS,
    SESSION_TIMEOUT_US,
};
pub use receiver::Receiver;
pub use recording_settings::{
    ACKNOWLEDGED_RANGES_VARIABLE, DEFAULT_ACKNOWLEDGED_RANGES, DEFAULT_WRITER_QUEUE_CHUNKS,
    DIRECTORY_VARIABLE, MAX_ACKNOWLEDGED_RANGES, RecordingSettings, WRITER_QUEUE_CHUNKS_VARIABLE,
};
pub use setting_error::SettingError;

pub use crate::srtla::{GroupHandle, IoFailure, ReceiverEvent, ReceiverIo};
