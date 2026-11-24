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

#[test]
fn a_restored_link_still_gets_an_even_share_of_the_live_stream() {
    let mut network = Network::new(&[
        LinkSpec::new(20 * MS, 50 * MBPS),
        LinkSpec::new(20 * MS, 50 * MBPS),
    ]);
    network.enable_recording(&recording_settings());
    let chunk = random_bytes(75_000, 5);
    let run = |network: &mut Network, seconds: u64, link_up: Option<bool>| {
        if let Some(up) = link_up {
            network.set_up(1, up);
        }
        for _ in 0..seconds * 10 {
            network.record(&chunk);
            network.run(100 * MS, 3 * MBPS);
        }
    };
    network.run(500 * MS, 0);
    run(&mut network, 5, None);
    run(&mut network, 5, Some(false));
    run(&mut network, 2, Some(true));
    network.take_live_counts();
    run(&mut network, 3, None);
    let counts = network.take_live_counts();
    assert!(
        counts[1] * 100 >= (counts[0] + counts[1]) * 45,
        "the upload must not tilt the live split towards the link that stayed: {counts:?}"
    );
}

/// The drain of `trace-drive` (CI 45253171813): three links of 1.4, 1.2 and 1.4 Mbit/s with
/// 100-packet queues, a trickle of live traffic (the encoder far below its maximum, so probing
/// wants more), and a backlog to upload. Probe runs used to stop the upload for a third of every
/// second (the run and its settling): it carried 3.0 of the 3.96 Mbit/s.
#[test]
fn a_backlog_fills_the_links_without_probe_runs_in_its_way() {
    const RATES_BPS: [u64; 3] = [1_416_000, 1_152_000, 1_392_000];
    const QUEUE_PACKETS: u64 = 100;
    const PACKET_BITS: u64 = 1_500 * 8;
    let mut network = Network::new(&RATES_BPS.map(|rate_bps| LinkSpec::new(40 * MS, rate_bps)));
    for (link, rate_bps) in RATES_BPS.iter().enumerate() {
        network.set_max_queue_us(link, QUEUE_PACKETS * PACKET_BITS * S / rate_bps);
    }
    network.enable_probing(ProbeTarget {
        stream_max_wire_bps: lab_target().stream_max_wire_bps,
        wanted_capacity_bps: 8 * MBPS,
    });
    network.enable_recording(&recording_settings());
    network.run(500 * MS, 0);
    network.record(&random_bytes(10_000_000, 7));
    network.run(5 * S, 50_000);
    let before = network
        .sender
        .recording()
        .map_or(0, RecordingUpload::delivered_bytes);
    network.run(10 * S, 50_000);
    let after = network
        .sender
        .recording()
        .map_or(0, RecordingUpload::delivered_bytes);
    let upload_bps = (after - before) * 8 / 10;
    let capacity_bps: u64 = RATES_BPS.iter().sum();
    assert!(
        upload_bps * 100 >= capacity_bps * 90,
        "the upload carried {upload_bps} bit/s of {capacity_bps}"
    );
}

/// `trace-drive` (CI 45278362883): a link went quiet for a dead second of its trace just as the
/// stream ended, its way back still answering: no stall. The upload's last chunks on it were lost, and no later
/// packet came to take them out of flight: they filled its window for good, and the link carried
/// nothing for the whole drain while its keepalives were answered.
#[test]
fn chunks_lost_as_a_link_goes_quiet_do_not_block_it() {
    let mut network = Network::new(&[
        LinkSpec::new(20 * MS, 2 * MBPS),
        LinkSpec::new(20 * MS, 2 * MBPS),
    ]);
    network.enable_recording(&recording_settings());
    network.run(500 * MS, 0);
    network.record(&random_bytes(5_000_000, 8));
    network.run(3 * S, 0);
    // Dead uplink, as netem's `loss 100%`: what is sent is lost, the way back still works.
    network.set_uplink_lost(0, true);
    network.run(250 * MS, 0);
    network.set_uplink_lost(0, false);
    network.run(3 * S, 0);
    network.take_data_counts();
    network.run(2 * S, 0);
    let counts = network.take_data_counts();
    assert!(
        counts.first().is_some_and(|&packets| packets > 100),
        "the link back carried {counts:?} packets"
    );
}

/// Live p99 right after a link drops from 8 to 1 Mbit/s at once, the live stream at 3.3 Mbit/s
/// over it and a 4 Mbit/s link, with the upload or without (`varying-bandwidth`).
fn live_p99_after_a_drop_us(settings: Option<&RecordingSettings>) -> u64 {
    const RATES_BPS: [u64; 2] = [4 * MBPS, 8 * MBPS];
    const QUEUE_PACKETS: u64 = 50;
    const PACKET_BITS: u64 = 1_500 * 8;
    let mut network = Network::new(&RATES_BPS.map(|rate_bps| LinkSpec::new(20 * MS, rate_bps)));
    let queue_us = |rate_bps: u64| QUEUE_PACKETS * PACKET_BITS * S / rate_bps;
    for (link, rate_bps) in RATES_BPS.iter().enumerate() {
        network.set_max_queue_us(link, queue_us(*rate_bps));
    }
    network.enable_probing(lab_target());
    if let Some(settings) = settings {
        network.enable_recording(settings);
    }
    let chunk = random_bytes(75_000, 9);
    network.run(500 * MS, 0);
    let run = |network: &mut Network| {
        for _ in 0..20 {
            if settings.is_some() {
                network.record(&chunk);
            }
            network.run(100 * MS, 3_300_000);
        }
    };
    for _ in 0..3 {
        run(&mut network);
    }
    network.take_latencies_us();
    network.set_rate(1, MBPS);
    network.set_max_queue_us(1, queue_us(MBPS));
    run(&mut network);
    let mut latencies_us = network.take_latencies_us();
    latencies_us.sort_unstable();
    latencies_us
        .get(latencies_us.len() * 99 / 100)
        .copied()
        .unwrap_or(u64::MAX)
}

/// While the live stream runs, the upload holds no more on a link than the live traffic does:
/// when the link's capacity drops at once, the live packets behind it wait about twice what
/// they did without the upload (CI 45278362883: 8 → 1 Mbit/s, the encoder backed off for 3 s).
#[test]
fn the_upload_holds_little_on_a_link_whose_capacity_drops() {
    let without_us = live_p99_after_a_drop_us(None);
    let with_us = live_p99_after_a_drop_us(Some(&recording_settings()));
    assert!(
        with_us * 2 <= without_us * 5,
        "live p99 after the drop: {with_us} µs with the upload, {without_us} µs without"
    );
}
