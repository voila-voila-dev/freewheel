//! The port between the receiver and the outside world.

use std::net::SocketAddrV4;

use super::group_handle::GroupHandle;
use super::receiver_event::ReceiverEvent;

/// An I/O operation that did not complete (a short send counts as a failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoFailure;

/// Everything [`Receiver`](super::Receiver) needs from outside. The server implements it with
/// UDP sockets; tests with a recording fake.
pub trait ReceiverIo {
    /// Sends a datagram to a sender's link, from the listening socket. A failure is reported and
    /// otherwise ignored, as upstream does.
    ///
    /// # Errors
    /// The datagram did not leave whole.
    fn send_to_link(&mut self, link: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure>;

    /// Opens the group's own socket to the SRT listener, on its first SRT packet. A failure
    /// removes the group.
    ///
    /// # Errors
    /// The socket could not be created or connected.
    fn open_srt(&mut self, group: GroupHandle) -> Result<(), IoFailure>;

    /// Forwards a datagram to the SRT listener on the group's socket. A failure removes the
    /// group.
    ///
    /// # Errors
    /// The datagram did not leave whole.
    fn send_to_srt(&mut self, group: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure>;

    /// Closes the group's SRT socket, when the group is removed.
    fn close_srt(&mut self, group: GroupHandle);

    /// Fills `bytes` with cryptographically random bytes (the receiver's half of a group id).
    ///
    /// # Errors
    /// No randomness available: the REG1 is refused.
    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure>;

    /// Keeps `bytes` of a session's recording at `offset` (the freewheel transport's
    /// background class, M1b step 6), without holding up the live stream: a write that cannot
    /// be taken now fails, and the sender sends the bytes again. Only called with a recording
    /// enabled; the default keeps nothing.
    ///
    /// # Errors
    /// The bytes were not taken (no room, no recording directory, the disk failed).
    fn write_recording(
        &mut self,
        session_id: u64,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), IoFailure> {
        let _ = (session_id, offset, bytes);
        Err(IoFailure)
    }

    /// Something worth a log line happened. Never called per packet, except for failed sends.
    fn report(&mut self, event: ReceiverEvent) {
        let _ = event;
    }
}
