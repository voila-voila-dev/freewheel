//! [`ReceiverIo`] on UDP sockets: one listening socket, one connected socket per group.

use std::fs::File;
use std::io::Read;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use crate::srtla::{GroupHandle, IoFailure, MAX_GROUPS, ReceiverEvent, ReceiverIo};

use super::log_line::log_line;

const RANDOMNESS_PATH: &str = "/dev/urandom";

/// The receiver's sockets. Sockets block; the event loop only reads those `poll` says are ready.
#[derive(Debug)]
pub struct UdpIo {
    listen: UdpSocket,
    srt_address: SocketAddrV4,
    srt_sockets: Vec<(GroupHandle, UdpSocket)>,
    randomness: Option<File>,
}

impl UdpIo {
    /// Wraps the listening socket; groups will be forwarded to `srt_address`.
    #[must_use]
    pub fn new(listen: UdpSocket, srt_address: SocketAddrV4) -> Self {
        Self {
            listen,
            srt_address,
            srt_sockets: Vec::with_capacity(MAX_GROUPS),
            randomness: File::open(RANDOMNESS_PATH).ok(),
        }
    }

    /// The socket the senders' links reach.
    #[must_use]
    pub const fn listen_socket(&self) -> &UdpSocket {
        &self.listen
    }

    /// Every open SRT socket, with its group.
    #[must_use]
    pub fn srt_sockets(&self) -> &[(GroupHandle, UdpSocket)] {
        &self.srt_sockets
    }

    /// A group's SRT socket, while it is open.
    #[must_use]
    pub fn srt_socket(&self, group: GroupHandle) -> Option<&UdpSocket> {
        self.srt_sockets
            .iter()
            .find(|(handle, _)| *handle == group)
            .map(|(_, socket)| socket)
    }
}

fn sent_whole(result: &std::io::Result<usize>, len: usize) -> Result<(), IoFailure> {
    match result {
        Ok(sent) if *sent == len => Ok(()),
        _ => Err(IoFailure),
    }
}

impl ReceiverIo for UdpIo {
    fn send_to_link(&mut self, link: SocketAddrV4, datagram: &[u8]) -> Result<(), IoFailure> {
        sent_whole(&self.listen.send_to(datagram, link), datagram.len())
    }

    fn open_srt(&mut self, group: GroupHandle) -> Result<(), IoFailure> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|_| IoFailure)?;
        socket.connect(self.srt_address).map_err(|_| IoFailure)?;
        self.srt_sockets.push((group, socket));
        Ok(())
    }

    fn send_to_srt(&mut self, group: GroupHandle, datagram: &[u8]) -> Result<(), IoFailure> {
        let socket = self.srt_socket(group).ok_or(IoFailure)?;
        sent_whole(&socket.send(datagram), datagram.len())
    }

    fn close_srt(&mut self, group: GroupHandle) {
        self.srt_sockets.retain(|(handle, _)| *handle != group);
    }

    fn fill_random(&mut self, bytes: &mut [u8]) -> Result<(), IoFailure> {
        let source = self.randomness.as_mut().ok_or(IoFailure)?;
        source.read_exact(bytes).map_err(|_| IoFailure)
    }

    fn report(&mut self, event: ReceiverEvent) {
        eprintln!("{}", log_line(&event));
    }
}
