//! The port between the sender and the outside world.

use std::net::{Ipv4Addr, SocketAddrV4};

use super::link_handle::LinkHandle;
use super::sender_event::SenderEvent;

/// An I/O operation that did not complete (a short send counts as a failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoFailure;

/// Everything [`Sender`](super::Sender) needs from outside. The server implements it with UDP
/// sockets; tests with a recording fake.
pub trait SenderIo {
    /// Opens the link's socket, bound to `source` so it leaves through that modem.
    ///
    /// # Errors
    /// The socket could not be created or bound (e.g. the modem has no such address yet).
    fn open_link(&mut self, link: LinkHandle, source: Ipv4Addr) -> Result<(), IoFailure>;

    /// Closes the link's socket, when the link is removed from the list.
    fn close_link(&mut self, link: LinkHandle);

    /// Sends a datagram to the receiver on that link's socket.
    ///
    /// # Errors
    /// The datagram did not leave whole (including a full socket buffer).
    fn send_on_link(
        &mut self,
        link: LinkHandle,
        receiver: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), IoFailure>;

    /// Sends a datagram back to the SRT caller. Failures are ignored, as upstream does.
    ///
    /// # Errors
    /// The datagram did not leave whole.
    fn send_to_srt(&mut self, caller: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure>;

    /// Something worth a log line happened. Never called per packet, except for failed sends.
    fn report(&mut self, event: SenderEvent) {
        let _ = event;
    }
}
