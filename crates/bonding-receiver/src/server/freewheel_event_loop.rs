//! The freewheel receiver's only thread: wait for a socket, hand its datagram over, send the
//! ACKs that are due, repeat.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bonding_protocol::freewheel::MAX_DATAGRAM_LEN;
use net_sys::{ReadinessSlot, wait_until_readable};

use freewheel_noise::ServerKeys;

use crate::freewheel::{GroupHandle, MAX_SESSIONS, Receiver, RecordingSettings};

use super::udp_io::UdpIo;

/// Longest wait without a datagram: ACKs held back must leave within
/// [`ACK_MAX_DELAY_US`](crate::freewheel::ACK_MAX_DELAY_US).
pub const FREEWHEEL_WAKE_INTERVAL: Duration = Duration::from_millis(2);

/// Serves the freewheel transport until `stop` is set. Same shape as the SRTLA loop
/// ([`serve`](super::serve)), with a microsecond clock. With `recording`, the recordings the
/// sessions upload are kept (`io` must then write them: [`UdpIo::with_recording`]). With
/// `keys`, encrypted (wire format v1, [`Receiver::enable_encryption`]); without, v0 in clear.
/// Nothing allocates per packet.
///
/// # Errors
/// Waiting on the sockets failed, which only happens when the process is out of resources.
pub fn serve_freewheel(
    io: &mut UdpIo,
    wake_interval: Duration,
    stop: &AtomicBool,
    recording: Option<&RecordingSettings>,
    keys: Option<ServerKeys>,
) -> io::Result<()> {
    let started = Instant::now();
    let now_us = || u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let mut receiver = Receiver::new();
    if let Some(settings) = recording {
        receiver.enable_recording(settings);
    }
    if let Some(keys) = keys {
        receiver.enable_encryption(keys);
    }
    let mut buffer = [0u8; MAX_DATAGRAM_LEN];
    let mut slots: Vec<ReadinessSlot> = Vec::with_capacity(1 + MAX_SESSIONS);
    let mut ready_sessions: Vec<GroupHandle> = Vec::with_capacity(MAX_SESSIONS);
    while !stop.load(Ordering::Relaxed) {
        slots.clear();
        slots.push(ReadinessSlot::new(io.listen_socket()));
        slots.extend(
            io.srt_sockets()
                .iter()
                .map(|(_, socket)| ReadinessSlot::new(socket)),
        );
        wait_until_readable(&mut slots, wake_interval)?;

        ready_sessions.clear();
        ready_sessions.extend(
            slots
                .iter()
                .skip(1)
                .zip(io.srt_sockets())
                .filter(|(slot, _)| slot.is_ready())
                .map(|(_, (session, _))| *session),
        );
        if slots.first().is_some_and(ReadinessSlot::is_ready)
            && let Ok((len, SocketAddr::V4(from))) = io.listen_socket().recv_from(&mut buffer)
            && let Some(datagram) = buffer.get(..len)
        {
            receiver.handle_link_datagram(now_us(), from, datagram, io);
        }
        for session in &ready_sessions {
            let Some(socket) = io.srt_socket(*session) else {
                continue;
            };
            match socket.recv(&mut buffer) {
                Ok(len) => {
                    if let Some(datagram) = buffer.get(..len) {
                        receiver.handle_srt_datagram(now_us(), *session, datagram, io);
                    }
                }
                Err(_) => receiver.handle_srt_failure(*session, io),
            }
        }
        receiver.housekeeping(now_us(), io);
    }
    Ok(())
}
