//! freewheel transport, receiving side (M1b, first candidate): gathers each session's links
//! and hands the SRT stream they carry to the SRT listener
//! (`docs/technical/protocol/freewheel.md`, "First candidate").
//!
//! [`Receiver`] decides; it reuses the SRTLA mode's [`ReceiverIo`] port (sockets, logs), a
//! session standing where SRTLA has a group. Nothing allocates per packet.

mod limits;
mod link;
mod receiver;
mod session;

pub use limits::{
    ACK_EVERY_PACKETS, ACK_MAX_DELAY_US, LINK_TIMEOUT_US, MAX_LINKS_PER_SESSION, MAX_SESSIONS,
    SESSION_TIMEOUT_US,
};
pub use receiver::Receiver;

pub use crate::srtla::{GroupHandle, IoFailure, ReceiverEvent, ReceiverIo};
