//! The server's only thread: wait for a socket, hand its datagram to the receiver, repeat.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bonding_protocol::srtla::MAX_DATAGRAM_LEN;
use net_sys::{ReadinessSlot, wait_until_readable};

use crate::srtla::{GroupHandle, MAX_GROUPS, Receiver};

use super::udp_io::UdpIo;

/// Longest wait without a datagram, so timeouts are checked at least this often (upstream: 1 s).
pub const WAKE_INTERVAL: Duration = Duration::from_secs(1);

/// Serves until `stop` is set (checked at least every `wake_interval`).
///
/// Like upstream, each wake reads one datagram from every ready socket, then runs the
/// housekeeping. Datagrams over [`MAX_DATAGRAM_LEN`] are truncated to it, as upstream's buffer
/// does. Nothing allocates per packet.
///
/// # Errors
/// Waiting on the sockets failed, which only happens when the process is out of resources.
pub fn serve(io: &mut UdpIo, wake_interval: Duration, stop: &AtomicBool) -> io::Result<()> {
    let started = Instant::now();
    let mut receiver = Receiver::new();
    let mut buffer = [0u8; MAX_DATAGRAM_LEN];
    let mut slots: Vec<ReadinessSlot> = Vec::with_capacity(1 + MAX_GROUPS);
    let mut ready_groups: Vec<GroupHandle> = Vec::with_capacity(MAX_GROUPS);
    while !stop.load(Ordering::Relaxed) {
        slots.clear();
        slots.push(ReadinessSlot::new(io.listen_socket()));
        slots.extend(
            io.srt_sockets()
                .iter()
                .map(|(_, socket)| ReadinessSlot::new(socket)),
        );
        wait_until_readable(&mut slots, wake_interval)?;
        let now_s = started.elapsed().as_secs();

        ready_groups.clear();
        ready_groups.extend(
            slots
                .iter()
                .skip(1)
                .zip(io.srt_sockets())
                .filter(|(slot, _)| slot.is_ready())
                .map(|(_, (group, _))| *group),
        );
        if slots.first().is_some_and(ReadinessSlot::is_ready)
            && let Ok((len, SocketAddr::V4(from))) = io.listen_socket().recv_from(&mut buffer)
            && let Some(datagram) = buffer.get(..len)
        {
            receiver.handle_link_datagram(now_s, from, datagram, io);
        }
        for group in &ready_groups {
            let Some(socket) = io.srt_socket(*group) else {
                continue;
            };
            match socket.recv(&mut buffer) {
                Ok(len) => {
                    if let Some(datagram) = buffer.get(..len) {
                        receiver.handle_srt_datagram(*group, datagram, io);
                    }
                }
                Err(_) => receiver.handle_srt_failure(*group, io),
            }
        }
        receiver.housekeeping(now_s, io);
    }
    Ok(())
}
