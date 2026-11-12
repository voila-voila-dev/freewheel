//! The sender's only thread: housekeeping, wait for a socket, hand its datagram over, repeat.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bonding_protocol::srtla::MAX_DATAGRAM_LEN;
use net_sys::{ReadinessSlot, wait_until_readable};

use crate::srtla::{HousekeepingOutcome, LinkHandle, Sender};

use super::serve_error::ServeError;
use super::source_list::read_source_list;
use super::udp_io::UdpIo;

/// Longest wait without a datagram, so housekeeping runs on time (upstream: 200 ms).
pub const WAKE_INTERVAL: Duration = Duration::from_millis(200);

/// Serves until `stop` is set (checked at least every `wake_interval`). When `reload` is set
/// (by `SIGHUP` in the program), it is cleared and the link list is reread from `source_list`.
///
/// Like upstream, each turn reloads if asked, runs the housekeeping, waits, then reads one
/// datagram from the SRT caller's socket and one from each ready link, in link order.
/// Datagrams over [`MAX_DATAGRAM_LEN`] are truncated to it. Nothing allocates per packet.
///
/// # Errors
/// See [`ServeError`].
pub fn serve(
    sender: &mut Sender,
    io: &mut UdpIo,
    source_list: &Path,
    wake_interval: Duration,
    reload: &AtomicBool,
    stop: &AtomicBool,
) -> Result<(), ServeError> {
    let started = Instant::now();
    let elapsed_ms = || u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let mut buffer = [0u8; MAX_DATAGRAM_LEN];
    let mut slots: Vec<ReadinessSlot> = Vec::new();
    let mut ready_links: Vec<LinkHandle> = Vec::new();
    let mut link_order: Vec<LinkHandle> = Vec::new();
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
        if sender.housekeeping(elapsed_ms(), io) == HousekeepingOutcome::GaveUp {
            return Err(ServeError::GaveUp);
        }

        slots.clear();
        slots.push(ReadinessSlot::new(io.listen_socket()));
        slots.extend(
            io.link_sockets()
                .iter()
                .map(|(_, socket)| ReadinessSlot::new(socket)),
        );
        wait_until_readable(&mut slots, wake_interval).map_err(ServeError::Wait)?;
        let now_ms = elapsed_ms();

        ready_links.clear();
        ready_links.extend(
            slots
                .iter()
                .skip(1)
                .zip(io.link_sockets())
                .filter(|(slot, _)| slot.is_ready())
                .map(|(_, (link, _))| *link),
        );
        if slots.first().is_some_and(ReadinessSlot::is_ready)
            && let Ok((len, SocketAddr::V4(from))) = io.listen_socket().recv_from(&mut buffer)
            && let Some(datagram) = buffer.get(..len)
        {
            sender.handle_srt_datagram(now_ms, from, datagram, io);
        }
        link_order.clear();
        link_order.extend(
            sender
                .links(now_ms)
                .map(|link| link.handle)
                .filter(|link| ready_links.contains(link)),
        );
        for link in &link_order {
            let Some(socket) = io.link_socket(*link) else {
                continue;
            };
            if let Ok((len, _)) = socket.recv_from(&mut buffer)
                && let Some(datagram) = buffer.get(..len)
            {
                sender.handle_link_datagram(now_ms, *link, datagram, io);
            }
        }
    }
    Ok(())
}
