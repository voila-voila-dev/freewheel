//! The recording upload end to end on a simulated network (M1b step 6): the real sender and
//! receiver, a recording in memory, the copy compared byte for byte.

mod freewheel_network;

use std::path::PathBuf;

use bitrate_control::ControllerSettings;
use bonding_sender::freewheel::{ProbeTarget, RecordingSettings, RecordingUpload};
use freewheel_network::{LinkSpec, Network};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;
const MBPS: u64 = 1_000_000;
const MAX_VIDEO_BPS: u32 = 3_000_000;

fn lab_target() -> ProbeTarget {
    let settings = ControllerSettings::lab(MAX_VIDEO_BPS, 128_000);
    ProbeTarget {
        stream_max_wire_bps: settings.wire_bps(MAX_VIDEO_BPS),
        wanted_capacity_bps: settings.capacity_for_bps(MAX_VIDEO_BPS),
    }
}

/// Bytes that never repeat a pattern a chunk long, so a chunk at the wrong offset shows.
fn random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state.to_le_bytes()[0]
        })
        .collect()
}

fn recording_settings() -> RecordingSettings {
    RecordingSettings::new(PathBuf::from("/unused"))
}

/// Two 8 Mbit/s links, a low live stream (the encoder below its ceiling: probing runs), and a
/// recording growing during `seconds` seconds by `bytes_per_second`.
fn probing_network() -> Network {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 8 * MBPS),
        LinkSpec::new(50 * MS, 8 * MBPS),
    ]);
    network.enable_probing(lab_target());
    network.enable_recording(&recording_settings());
    network
}

#[test]
fn probe_runs_carry_the_recording_instead_of_padding() {
    let mut network = probing_network();
    let recording = random_bytes(60_000, 1);
    network.run(500 * MS, 0);
    for second in 0..3 {
        network.record(&recording[second * 20_000..(second + 1) * 20_000]);
        network.run(S, 400_000);
    }
    network.run(20 * S, 400_000);
    assert_eq!(network.recording_copy(), recording);
    assert!(
        network
            .sender
            .recording()
            .is_some_and(RecordingUpload::complete),
        "the sender knows it is delivered"
    );
}

#[test]
fn an_outage_of_every_link_pauses_the_upload_and_it_resumes() {
    let mut network = probing_network();
    let recording = random_bytes(80_000, 2);
    network.run(500 * MS, 0);
    network.record(&recording[..40_000]);
    network.run(2 * S, 400_000);
    for link in 0..2 {
        network.set_up(link, false);
    }
    network.record(&recording[40_000..]);
    network.run(4 * S, 400_000);
    for link in 0..2 {
        network.set_up(link, true);
    }
    network.run(30 * S, 400_000);
    assert_eq!(network.recording_copy(), recording);
    let lost = network
        .sender
        .recording()
        .map_or(0, RecordingUpload::lost_chunks);
    assert!(lost > 0, "chunks were in flight when the links went down");
}

#[test]
fn the_live_stream_loses_nothing_to_the_upload() {
    let mut network = probing_network();
    network.run(500 * MS, 0);
    network.record(&random_bytes(200_000, 3));
    network.run(10 * S, 400_000);
    network.run(500 * MS, 0);
    assert_eq!(
        network.delivered().len(),
        usize::try_from(network.sent()).unwrap_or(0)
    );
}

/// 99th percentile of `latencies_us`, in microseconds.
fn p99_us(mut latencies_us: Vec<u64>) -> u64 {
    latencies_us.sort_unstable();
    let index = latencies_us.len() * 99 / 100;
    latencies_us.get(index).copied().unwrap_or(0)
}

/// `links`, the live stream at `stream_bps` for `seconds`, with or without a recording growing
/// at `recording_bps` meanwhile, then as long again with the stream at a trickle (the drain).
/// Returns the network and the live stream's p99 latency while it ran.
fn spare_capacity_run(
    links: &[LinkSpec],
    stream_bps: u64,
    recording_bps: Option<u64>,
    seconds: u64,
) -> (Network, u64) {
    let mut network = Network::new(links);
    if recording_bps.is_some() {
        network.enable_recording(&recording_settings());
    }
    network.run(500 * MS, 0);
    let recording = random_bytes(
        usize::try_from(recording_bps.unwrap_or(0) * seconds / 8).unwrap_or(0),
        4,
    );
    let per_tick = recording.len() / usize::try_from(seconds * 10).unwrap_or(1);
    let _ = network.take_latencies_us();
    for tick in 0..seconds * 10 {
        let at = usize::try_from(tick).unwrap_or(0) * per_tick;
        network.record(recording.get(at..at + per_tick).unwrap_or_default());
        network.run(100 * MS, stream_bps);
    }
    let p99 = p99_us(network.take_latencies_us());
    network.run(seconds * S, 50_000);
    (network, p99)
}

#[test]
fn spare_room_carries_the_recording_while_the_encoder_is_at_its_ceiling() {
    let links = [
        LinkSpec::new(30 * MS, 8 * MBPS),
        LinkSpec::new(50 * MS, 8 * MBPS),
    ];
    let (without, p99_without) = spare_capacity_run(&links, 3_400_000, None, 10);
    let (with, p99_with) = spare_capacity_run(&links, 3_400_000, Some(6 * MBPS), 10);
    let delivered = with
        .sender
        .recording()
        .map_or(0, RecordingUpload::delivered_bytes);
    assert_eq!(
        with.recording_copy(),
        with.recording(),
        "{delivered} bytes delivered"
    );
    assert_eq!(with.delivered().len(), without.delivered().len());
    assert!(
        p99_with <= p99_without + 20 * MS,
        "live p99 {p99_with} µs with the upload, {p99_without} µs without"
    );
}

#[test]
fn on_links_the_live_stream_nearly_fills_the_upload_takes_only_what_is_left() {
    let links = [
        LinkSpec::new(30 * MS, 2 * MBPS),
        LinkSpec::new(50 * MS, 2 * MBPS),
    ];
    let (without, p99_without) = spare_capacity_run(&links, 3_000_000, None, 10);
    let (with, p99_with) = spare_capacity_run(&links, 3_000_000, Some(6 * MBPS), 10);
    assert_eq!(
        with.delivered().len(),
        without.delivered().len(),
        "the live stream loses nothing to the upload"
    );
    assert!(
        p99_with <= p99_without + 20 * MS,
        "live p99 {p99_with} µs with the upload, {p99_without} µs without"
    );
    let delivered = with
        .sender
        .recording()
        .map_or(0, RecordingUpload::delivered_bytes);
    assert!(delivered > 0, "the spare megabit carried some of it");
}
