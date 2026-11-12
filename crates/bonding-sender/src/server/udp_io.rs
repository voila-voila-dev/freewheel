//! [`SenderIo`] on UDP sockets: one for the SRT caller, one per link bound to its modem.

use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use net_sys::set_send_buffer_bytes;

use crate::srtla::{IoFailure, LinkHandle, SenderEvent, SenderIo};

use super::log_line::log_line;

/// Send buffer asked for each link socket, as upstream does (Linux grants at most
/// `net.core.wmem_max`, doubled).
pub const SEND_BUFFER_BYTES: usize = 8 * 1024 * 1024;

/// The sender's sockets. Link sockets do not block, like upstream's: a full buffer is a failed
/// send. The event loop only reads sockets `poll` says are ready.
#[derive(Debug)]
pub struct UdpIo {
    listen: UdpSocket,
    links: Vec<(LinkHandle, UdpSocket)>,
}

impl UdpIo {
    /// Wraps the socket the SRT caller sends to.
    #[must_use]
    pub const fn new(listen: UdpSocket) -> Self {
        Self {
            listen,
            links: Vec::new(),
        }
    }

    /// The socket the SRT caller sends to.
    #[must_use]
    pub const fn listen_socket(&self) -> &UdpSocket {
        &self.listen
    }

    /// Every open link socket, with its link.
    #[must_use]
    pub fn link_sockets(&self) -> &[(LinkHandle, UdpSocket)] {
        &self.links
    }

    /// A link's socket, while it is open.
    #[must_use]
    pub fn link_socket(&self, link: LinkHandle) -> Option<&UdpSocket> {
        self.links
            .iter()
            .find(|(handle, _)| *handle == link)
            .map(|(_, socket)| socket)
    }
}

fn sent_whole(result: &std::io::Result<usize>, len: usize) -> Result<(), IoFailure> {
    match result {
        Ok(sent) if *sent == len => Ok(()),
        _ => Err(IoFailure),
    }
}

impl SenderIo for UdpIo {
    fn open_link(&mut self, link: LinkHandle, source: Ipv4Addr) -> Result<(), IoFailure> {
        let socket = UdpSocket::bind((source, 0)).map_err(|_| IoFailure)?;
        socket.set_nonblocking(true).map_err(|_| IoFailure)?;
        set_send_buffer_bytes(&socket, SEND_BUFFER_BYTES).map_err(|_| IoFailure)?;
        self.close_link(link);
        self.links.push((link, socket));
        Ok(())
    }

    fn close_link(&mut self, link: LinkHandle) {
        self.links.retain(|(handle, _)| *handle != link);
    }

    fn send_on_link(
        &mut self,
        link: LinkHandle,
        receiver: SocketAddrV4,
        datagram: &[u8],
    ) -> Result<(), IoFailure> {
        let socket = self.link_socket(link).ok_or(IoFailure)?;
        sent_whole(&socket.send_to(datagram, receiver), datagram.len())
    }

    fn send_to_srt(&mut self, caller: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        sent_whole(&self.listen.send_to(datagram, caller), datagram.len())
    }

    fn report(&mut self, event: SenderEvent) {
        eprintln!("{}", log_line(&event));
    }
}
