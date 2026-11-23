//! With the recording off, the transport is exactly step 5's (M1b step 6, `plans/m1b-step-6.md`,
//! "Settings"): every datagram either side sends, its time and its link, as digested on step 5's
//! code (`main` at `7ad1e8c`) over a run with probing, a cut of every link, a slow link and SRT
//! answers.

mod freewheel_network;

use bitrate_control::ControllerSettings;
use bonding_sender::freewheel::ProbeTarget;
use freewheel_network::{LinkSpec, Network};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;
const MBPS: u64 = 1_000_000;
const MAX_VIDEO_BPS: u32 = 3_000_000;
/// [`Network::wire_digest`] of [`step_5_run`] on step 5's code.
const STEP_5_DIGEST: u64 = 15_964_225_347_698_857_249;

/// Probing on, three uneven links, all cut for 2 s, one slowed down, SRT answering.
fn step_5_run() -> Network {
    let mut network = Network::new(&[
        LinkSpec::new(30 * MS, 6 * MBPS),
        LinkSpec::new(50 * MS, 3 * MBPS),
        LinkSpec::new(80 * MS, 2 * MBPS),
    ]);
    let settings = ControllerSettings::lab(MAX_VIDEO_BPS, 128_000);
    network.enable_probing(ProbeTarget {
        stream_max_wire_bps: settings.wire_bps(MAX_VIDEO_BPS),
        wanted_capacity_bps: settings.capacity_for_bps(MAX_VIDEO_BPS),
    });
    network.run(500 * MS, 0);
    network.run(3 * S, 1_500_000);
    network.srt_answer();
    for link in 0..3 {
        network.set_up(link, false);
    }
    network.run(2 * S, 1_500_000);
    for link in 0..3 {
        network.set_up(link, true);
    }
    network.run(3 * S, 2_500_000);
    network.set_rate(0, MBPS);
    network.srt_answer();
    network.run(3 * S, 2_500_000);
    network
}

#[test]
fn without_a_recording_the_wire_is_step_5s() {
    let network = step_5_run();
    assert!(network.delivered().len() > 1_000, "the stream ran");
    assert!(network.sender.padding_bytes() > 0, "probing ran");
    assert_eq!(
        network.wire_digest(),
        STEP_5_DIGEST,
        "the sender or the receiver sends other bytes, or at other times, than step 5"
    );
}
