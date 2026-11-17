//! The freewheel sender's only thread: housekeeping, wait for a socket, hand its datagram
//! over, repeat.

use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bonding_protocol::freewheel::{MAX_DATAGRAM_LEN, SessionId};
use net_sys::{ReadinessSlot, wait_until_readable};

use crate::freewheel::{HOUSEKEEPING_PERIOD_US, LinkHandle, Sender};

use super::serve_error::ServeError;
use super::source_list::read_source_list;
use super::udp_io::UdpIo;

const RANDOMNESS_PATH: &str = "/dev/urandom";

/// Longest wait without a datagram, so housekeeping (keepalives, stall checks) runs on time.
pub const FREEWHEEL_WAKE_INTERVAL: Duration = Duration::from_micros(HOUSEKEEPING_PERIOD_US);

/// A random session id, from the system's randomness.
///
/// # Errors
/// No randomness available.
pub fn random_session_id() -> std::io::Result<SessionId> {
    let mut bytes = [0u8; 8];
    File::open(RANDOMNESS_PATH)?.read_exact(&mut bytes)?;
    Ok(SessionId::from_be_bytes(bytes))
}

/// Serves the freewheel transport until `stop` is set. Same shape as the SRTLA loop
/// ([`serve`](super::serve)): reload on `reload`, housekeeping, wait, then one datagram from
/// the SRT caller and one from each ready link. Nothing allocates per packet.
///
/// # Errors
/// See [`ServeError`].
pub fn serve_freewheel(
    sender: &mut Sender,
    io: &mut UdpIo,
    source_list: &Path,
    wake_interval: Duration,
    reload: &AtomicBool,
    stop: &AtomicBool,
) -> Result<(), ServeError> {
    let started = Instant::now();
    let now_us = || u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let mut buffer = [0u8; MAX_DATAGRAM_LEN];
    let mut slots: Vec<ReadinessSlot> = Vec::new();
    let mut ready_links: Vec<LinkHandle> = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        if reload.swap(false, Ordering::Relaxed) {
            let sources = read_source_list(source_list).map_err(|error| {
                ServeError::SourceListUnreadable {
                    path: source_list.to_path_buf(),
                    error,
                }
            })?;
            sender.reload_sources(&sources, io);
        }
        sender.housekeeping(now_us(), io);

        slots.clear();
        slots.push(ReadinessSlot::new(io.listen_socket()));
        slots.extend(
            io.link_sockets()
                .map(|(_, socket)| ReadinessSlot::new(socket)),
        );
        wait_until_readable(&mut slots, wake_interval).map_err(ServeError::Wait)?;

        ready_links.clear();
        ready_links.extend(
            slots
                .iter()
                .skip(1)
                .zip(io.link_sockets())
                .filter(|(slot, _)| slot.is_ready())
                .map(|(_, (link, _))| link),
        );
        if slots.first().is_some_and(ReadinessSlot::is_ready)
            && let Ok((len, SocketAddr::V4(from))) = io.listen_socket().recv_from(&mut buffer)
            && let Some(datagram) = buffer.get(..len)
        {
            sender.handle_srt_datagram(now_us(), from, datagram, io);
        }
        for link in &ready_links {
            let Some(socket) = io.link_socket(*link) else {
                continue;
            };
            if let Ok((len, _)) = socket.recv_from(&mut buffer)
                && let Some(datagram) = buffer.get(..len)
            {
                sender.handle_link_datagram(now_us(), *link, datagram, io);
            }
        }
    }
    Ok(())
}
