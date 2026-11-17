//! When each video frame of an MPEG-TS stream went by: fed the stream as it flows (UDP
//! datagrams, pipe reads), it reports every frame's timestamp and the time its last byte passed.
//!
//! Used on both ends of the lab: what the encoder sent (a capture of its UDP output) and what
//! the SRT listener delivered (its output, timed as it is written). Their difference per frame is
//! the latency the bonding added.

use crate::FrameTime;

/// MPEG-TS packet size (ISO/IEC 13818-1).
const PACKET_BYTES: usize = 188;
const SYNC_BYTE: u8 = 0x47;
const PAT_PID: u16 = 0;
/// Stream types of a video elementary stream in the PMT: MPEG-2, H.264, HEVC.
const VIDEO_STREAM_TYPES: [u8; 3] = [0x02, 0x1B, 0x24];
/// Bytes of a PSI section header before its body, and of its trailing CRC.
const SECTION_HEADER_BYTES: usize = 3;
const SECTION_CRC_BYTES: usize = 4;
/// PES header: the PTS sits after the start code, stream id, length, flags and header length.
const PES_PTS_OFFSET: usize = 9;
const PES_PTS_BYTES: usize = 5;
const PES_PTS_FLAG: u8 = 0x80;

/// Follows one MPEG-TS stream and reports when each video frame was complete.
#[derive(Debug, Default)]
pub struct FrameClock {
    /// Bytes of a packet split across two pushes.
    partial: Vec<u8>,
    pmt_pid: Option<u16>,
    video_pid: Option<u16>,
    /// The frame being received: its timestamp and when its last packet so far went by.
    current: Option<FrameTime>,
}

impl FrameClock {
    /// A clock that has not seen any byte yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds `bytes` that went by at `at_us` (microseconds since the Unix epoch). Every frame
    /// they complete is passed to `completed`, in stream order.
    pub fn push(&mut self, at_us: u64, bytes: &[u8], completed: &mut impl FnMut(FrameTime)) {
        let mut rest = bytes;
        if !self.partial.is_empty() {
            let missing = PACKET_BYTES - self.partial.len();
            let taken = missing.min(rest.len());
            self.partial.extend_from_slice(&rest[..taken]);
            rest = &rest[taken..];
            if self.partial.len() < PACKET_BYTES {
                return;
            }
            let packet = std::mem::take(&mut self.partial);
            self.packet(at_us, &packet, completed);
        }
        while !rest.is_empty() {
            if rest[0] != SYNC_BYTE {
                // Lost sync (a damaged write): skip to the next sync byte.
                let skip = rest
                    .iter()
                    .position(|byte| *byte == SYNC_BYTE)
                    .unwrap_or(rest.len());
                rest = &rest[skip..];
                continue;
            }
            if rest.len() < PACKET_BYTES {
                self.partial.extend_from_slice(rest);
                return;
            }
            let (packet, next) = rest.split_at(PACKET_BYTES);
            self.packet(at_us, packet, completed);
            rest = next;
        }
    }

    /// The stream ended: reports the frame still being received.
    pub fn finish(&mut self, completed: &mut impl FnMut(FrameTime)) {
        if let Some(frame) = self.current.take() {
            completed(frame);
        }
    }

    fn packet(&mut self, at_us: u64, packet: &[u8], completed: &mut impl FnMut(FrameTime)) {
        let unit_start = packet[1] & 0x40 != 0;
        let pid = (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]);
        let Some(payload) = payload(packet) else {
            return;
        };
        if pid == PAT_PID && unit_start {
            self.pmt_pid = section(payload).and_then(first_program_pid);
        } else if Some(pid) == self.pmt_pid && unit_start {
            self.video_pid = section(payload).and_then(first_video_pid);
        } else if Some(pid) == self.video_pid {
            if unit_start && let Some(pts) = pes_pts(payload) {
                if let Some(frame) = self.current.take() {
                    completed(frame);
                }
                self.current = Some(FrameTime { pts, at_us });
            } else if let Some(frame) = &mut self.current {
                frame.at_us = at_us;
            }
        }
    }
}

/// The packet's payload, after its adaptation field; `None` when it has none.
fn payload(packet: &[u8]) -> Option<&[u8]> {
    let adaptation = (packet[3] >> 4) & 0b11;
    if adaptation & 0b01 == 0 {
        return None;
    }
    let start = if adaptation & 0b10 == 0 {
        4
    } else {
        5 + usize::from(*packet.get(4)?)
    };
    packet.get(start..)
}

/// The body of the PSI section starting in this payload (after its pointer field), without the
/// CRC.
fn section(payload: &[u8]) -> Option<&[u8]> {
    let pointer = usize::from(*payload.first()?);
    let section = payload.get(1 + pointer..)?;
    let length = (usize::from(*section.get(1)? & 0x0F) << 8) | usize::from(*section.get(2)?);
    section.get(..(SECTION_HEADER_BYTES + length).checked_sub(SECTION_CRC_BYTES)?)
}

/// PID of the first program's PMT in a PAT section.
fn first_program_pid(section: &[u8]) -> Option<u16> {
    section.get(8..)?.chunks_exact(4).find_map(|entry| {
        let program = u16::from_be_bytes([entry[0], entry[1]]);
        (program != 0).then(|| (u16::from(entry[2] & 0x1F) << 8) | u16::from(entry[3]))
    })
}

/// PID of the first video stream in a PMT section.
fn first_video_pid(section: &[u8]) -> Option<u16> {
    let program_info =
        (usize::from(*section.get(10)? & 0x0F) << 8) | usize::from(*section.get(11)?);
    let mut offset = 12 + program_info;
    while let Some(entry) = section.get(offset..offset + 5) {
        let pid = (u16::from(entry[1] & 0x1F) << 8) | u16::from(entry[2]);
        if VIDEO_STREAM_TYPES.contains(&entry[0]) {
            return Some(pid);
        }
        let info = (usize::from(entry[3] & 0x0F) << 8) | usize::from(entry[4]);
        offset += 5 + info;
    }
    None
}

/// The presentation timestamp of the PES packet starting in this payload.
fn pes_pts(payload: &[u8]) -> Option<i64> {
    if payload.get(..3)? != [0, 0, 1] || payload.get(7)? & PES_PTS_FLAG == 0 {
        return None;
    }
    let pts = payload.get(PES_PTS_OFFSET..PES_PTS_OFFSET + PES_PTS_BYTES)?;
    Some(
        (i64::from((pts[0] >> 1) & 0x07) << 30)
            | (i64::from(pts[1]) << 22)
            | (i64::from(pts[2] >> 1) << 15)
            | (i64::from(pts[3]) << 7)
            | i64::from(pts[4] >> 1),
    )
}
