//! One control packet worth seeing on a timeline (registration, keepalive, errors).

use std::net::SocketAddrV4;

use bonding_protocol::srtla::PacketType;

/// One control packet worth seeing on a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SrtlaEvent {
    /// Time since the first packet of the capture, in milliseconds.
    pub at_ms: u64,
    /// The sender-side link it travelled on.
    pub link: SocketAddrV4,
    /// `true` when sent by the sender (box → server).
    pub uplink: bool,
    /// What it was.
    pub kind: PacketType,
    /// UDP payload length, in bytes.
    pub bytes: usize,
}
