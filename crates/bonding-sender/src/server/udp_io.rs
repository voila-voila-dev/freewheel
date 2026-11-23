//! [`SenderIo`] on UDP sockets: one for the SRT caller, one per link bound to its modem.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use net_sys::set_send_buffer_bytes;

use crate::srtla::{IoFailure, LinkHandle, SenderEvent, SenderIo};

use super::log_line::log_line;
use super::recording_file::RecordingFile;

/// Send buffer asked for each link socket, as upstream does (Linux grants at most
/// `net.core.wmem_max`, doubled).
pub const SEND_BUFFER_BYTES: usize = 8 * 1024 * 1024;

/// The sender's sockets. Link sockets do not block, like upstream's: a full buffer is a failed
/// send. The event loop only reads sockets `poll` says are ready.
///
/// Unlike upstream, a link socket is connected to the receiver it sends to: the kernel then
/// keeps the route instead of looking it up for every packet, the largest cost of `sendto` at
/// high bitrates. Connecting also filters what the socket receives to that receiver, which is
/// all upstream ever expects there.
#[derive(Debug)]
pub struct UdpIo {
    listen: UdpSocket,
    links: Vec<LinkSocket>,
    recording: Option<RecordingFile>,
}

/// One link's socket, and the receiver it is connected to (if any yet).
#[derive(Debug)]
struct LinkSocket {
    handle: LinkHandle,
    socket: UdpSocket,
    peer: Option<SocketAddrV4>,
}

impl UdpIo {
    /// Wraps the socket the SRT caller sends to.
    #[must_use]
    pub const fn new(listen: UdpSocket) -> Self {
        Self {
            listen,
            links: Vec::new(),
            recording: None,
        }
    }

    /// Reads the recording to upload from `recording`.
    #[must_use]
    pub fn with_recording(mut self, recording: RecordingFile) -> Self {
        self.recording = Some(recording);
        self
    }

    /// The socket the SRT caller sends to.
    #[must_use]
    pub const fn listen_socket(&self) -> &UdpSocket {
        &self.listen
    }

    /// Every open link socket, with its link.
    pub fn link_sockets(&self) -> impl Iterator<Item = (LinkHandle, &UdpSocket)> {
        self.links.iter().map(|link| (link.handle, &link.socket))
    }

    /// A link's socket, while it is open.
    #[must_use]
    pub fn link_socket(&self, link: LinkHandle) -> Option<&UdpSocket> {
        self.links
            .iter()
            .find(|candidate| candidate.handle == link)
            .map(|link| &link.socket)
    }
}

fn sent_whole(result: &std::io::Result<usize>, len: usize) -> Result<(), IoFailure> {
    match result {
        Ok(sent) if *sent == len => Ok(()),
        _ => Err(IoFailure),
    }
}

/// Sends on a socket connected to `receiver`, connecting it first if it is not yet.
///
/// A connected UDP socket reports an ICMP error (the receiver's port was closed, e.g. while it
/// restarts) on its next send, which then fails without sending. An unconnected socket, as
/// upstream uses, never sees them: the send is tried once more, so the link behaves the same.
fn send_connected(
    link: &mut LinkSocket,
    receiver: SocketAddrV4,
    datagram: &[u8],
) -> Result<(), IoFailure> {
    if link.peer != Some(receiver) {
        link.socket.connect(receiver).map_err(|_| IoFailure)?;
        link.peer = Some(receiver);
    }
    let result = match link.socket.send(datagram) {
        Err(error) if error.kind() == ErrorKind::ConnectionRefused => link.socket.send(datagram),
        result => result,
    };
    sent_whole(&result, datagram.len())
}

impl SenderIo for UdpIo {
    fn open_link(&mut self, link: LinkHandle, source: Ipv4Addr) -> Result<(), IoFailure> {
        let socket = UdpSocket::bind((source, 0)).map_err(|_| IoFailure)?;
        socket.set_nonblocking(true).map_err(|_| IoFailure)?;
        set_send_buffer_bytes(&socket, SEND_BUFFER_BYTES).map_err(|_| IoFailure)?;
        self.close_link(link);
        self.links.push(LinkSocket {
            handle: link,
            socket,
            peer: None,
        });
        Ok(())
    }

    fn close_link(&mut self, link: LinkHandle) {
        self.links.retain(|candidate| candidate.handle != link);
    }

    fn send_on_link(
        &mut self,
        link: LinkHandle,
        receiver: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), IoFailure> {
        let link = self
            .links
            .iter_mut()
            .find(|candidate| candidate.handle == link)
            .ok_or(IoFailure)?;
        send_connected(link, receiver, datagram)
    }

    fn send_to_srt(&mut self, caller: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        sent_whole(&self.listen.send_to(datagram, caller), datagram.len())
    }

    fn recording_bytes(&mut self) -> Option<u64> {
        self.recording.as_mut().and_then(RecordingFile::len_bytes)
    }

    fn read_recording(&mut self, offset: u64, out: &mut [u8]) -> Result<(), IoFailure> {
        self.recording
            .as_ref()
            .ok_or(IoFailure)?
            .read_at(offset, out)
    }

    fn report(&mut self, event: SenderEvent) {
        eprintln!("{}", log_line(&event));
    }
}
