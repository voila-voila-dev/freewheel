//! freewheel transport, receiving side (M1b, first candidate): gathers each session's links
//! and hands the SRT stream they carry to the SRT listener
//! (`docs/technical/protocol/freewheel.md`, "First candidate").
//!
//! [`Receiver`] decides; it reuses the SRTLA mode's [`ReceiverIo`] port (sockets, logs), a
//! session standing where SRTLA has a group. Nothing allocates per packet.

mod limits;
mod link;
mod received_window;
mod receiver;
mod session;

pub use limits::{
    ACK_EVERY_PACKETS, ACK_MAX_DELAY_US, ACK_RANGES, LINK_TIMEOUT_US, MAX_LINKS_PER_SESSION,
    MAX_SESSIONS, RECEIVED_WINDOW_SEQUENCES, SESSION_TIMEOUT_US,
};
pub use received_window::{Arrival, ReceivedWindow};
pub use receiver::Receiver;

pub use crate::srtla::{GroupHandle, IoFailure, ReceiverEvent, ReceiverIo};
