//! `FrameClock`: when each video frame of an MPEG-TS stream went by.

use packet_capture::{FrameClock, FrameTime};

const PACKET_BYTES: usize = 188;
const PMT_PID: u16 = 0x1000;
const VIDEO_PID: u16 = 0x100;
const AUDIO_PID: u16 = 0x101;

/// One TS packet: header, payload, stuffed with 0xFF.
fn packet(pid: u16, unit_start: bool, payload: &[u8]) -> Vec<u8> {
    let mut packet = vec![
        0x47,
        (u8::from(unit_start) << 6) | u8::try_from(pid >> 8).unwrap_or(0),
        u8::try_from(pid & 0xFF).unwrap_or(0),
        0x10,
    ];
    packet.extend_from_slice(payload);
    packet.resize(PACKET_BYTES, 0xFF);
    packet
}

/// A PSI section (pointer field, header, body, dummy CRC).
fn section(table_id: u8, body: &[u8]) -> Vec<u8> {
    let length = body.len() + 5 + 4;
    let mut section = vec![
        0,
        table_id,
        0xB0 | u8::try_from(length >> 8).unwrap_or(0),
        u8::try_from(length & 0xFF).unwrap_or(0),
        0,
        1,
        0xC1,
        0,
        0,
    ];
    section.extend_from_slice(body);
    section.extend_from_slice(&[0; 4]);
    section
}

fn pat() -> Vec<u8> {
    packet(
        0,
        true,
        &section(
            0,
            &[
                0,
                1,
                0xE0 | u8::try_from(PMT_PID >> 8).unwrap_or(0),
                u8::try_from(PMT_PID & 0xFF).unwrap_or(0),
            ],
        ),
    )
}

fn pmt() -> Vec<u8> {
    let body = [
        0xE1, 0x00, 0xF0, 0x00, // PCR PID, no program info
        0x0F, 0xE1, 0x01, 0xF0, 0x00, // AAC audio first
        0x1B, 0xE1, 0x00, 0xF0, 0x00, // then H.264 video
    ];
    packet(PMT_PID, true, &section(2, &body))
}

/// The first packet of a video PES carrying `pts`.
fn pes_start(pts: i64) -> Vec<u8> {
    let byte = |shift: u32| u8::try_from((pts >> shift) & 0xFF).unwrap_or(0);
    let header = [
        0,
        0,
        1,
        0xE0,
        0,
        0,
        0x80,
        0x80,
        5,
        0x21 | (u8::try_from((pts >> 29) & 0x0E).unwrap_or(0)),
        byte(22),
        (byte(14) & 0xFE) | 1,
        byte(7),
        (byte(0) << 1) | 1,
    ];
    packet(VIDEO_PID, true, &header)
}

fn clock_over(chunks: &[(u64, Vec<u8>)]) -> Vec<FrameTime> {
    let mut frames = Vec::new();
    let mut clock = FrameClock::new();
    for (at_us, bytes) in chunks {
        clock.push(*at_us, bytes, &mut |frame| frames.push(frame));
    }
    clock.finish(&mut |frame| frames.push(frame));
    frames
}

#[test]
fn each_frame_is_dated_by_its_last_packet() {
    let frames = clock_over(&[
        (1_000, [pat(), pmt(), pes_start(90_000)].concat()),
        (2_000, packet(VIDEO_PID, false, &[1; 10])),
        (2_500, packet(AUDIO_PID, true, &[0, 0, 1, 0xC0])),
        (
            3_000,
            [packet(VIDEO_PID, false, &[2; 10]), pes_start(93_000)].concat(),
        ),
        (4_000, packet(VIDEO_PID, false, &[3; 10])),
    ]);
    assert_eq!(
        frames,
        vec![
            FrameTime {
                pts: 90_000,
                at_us: 3_000
            },
            FrameTime {
                pts: 93_000,
                at_us: 4_000
            },
        ]
    );
}

#[test]
fn packets_split_across_reads_are_reassembled() {
    let stream = [
        pat(),
        pmt(),
        pes_start(8_589_934_000),
        packet(VIDEO_PID, false, &[1; 10]),
        pes_start(3_000),
    ]
    .concat();
    let chunks: Vec<(u64, Vec<u8>)> = stream
        .chunks(100)
        .zip(1_u64..)
        .map(|(bytes, at)| (at, bytes.to_vec()))
        .collect();
    let frames = clock_over(&chunks);
    assert_eq!(
        frames.iter().map(|frame| frame.pts).collect::<Vec<_>>(),
        vec![8_589_934_000, 3_000]
    );
    assert_eq!(
        frames[0].at_us, 8,
        "last packet of the first frame ends in read 8 (752 bytes)"
    );
}

#[test]
fn garbage_before_the_stream_is_skipped() {
    let frames = clock_over(&[(5, [vec![1, 2, 3], pat(), pmt(), pes_start(42)].concat())]);
    assert_eq!(frames, vec![FrameTime { pts: 42, at_us: 5 }]);
}

#[test]
fn no_frame_without_a_program_table() {
    assert!(clock_over(&[(1, pes_start(42))]).is_empty());
}

#[test]
fn times_round_trip_through_text() {
    let frame = FrameTime {
        pts: 123,
        at_us: 1_763_000_000_000_000,
    };
    assert_eq!(
        FrameTime::parse_all(&format!("{}\nbad line\n", frame.line())),
        vec![frame]
    );
}
