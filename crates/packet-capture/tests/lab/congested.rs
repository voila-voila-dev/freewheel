//! `congested`: with less capacity than bitrate, upstream has no answer: video is dropped.

use crate::support::{load, modem, share_percent};

#[test]
fn too_little_capacity_drops_video() {
    let Some(run) = load("congested") else { return };
    for k in [1, 2] {
        assert!(share_percent(&run, modem(k), 2, 28) >= 30, "link {k}");
    }
    // Without adaptive bitrate in front, SRT gives up on many late packets.
    assert!(
        run.srt.dropped_ppm() > 50_000,
        "dropped {} ppm",
        run.srt.dropped_ppm()
    );
}
