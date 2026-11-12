//! `real-ride`: two minutes of uneven, lossy, jittery modems with outages. Upstream leans on
//! the best link, starves the others, and loses video when it has to fall back on them.

use crate::support::{data_packets, load, mark_s, modem, share_percent};

const RIDE_END_S: usize = 120;
/// Full-size data packets per second that fit in link 3's 2 Mbit/s.
const LINK_3_PACKETS_PER_SECOND: u64 = 180;

#[test]
fn the_best_link_carries_almost_everything() {
    let Some(run) = load("real-ride") else { return };
    // Capacities are 6, 3 and 2 Mbit/s: a fair split would be about 55, 27 and 18 %.
    assert!(share_percent(&run, modem(1), 2, RIDE_END_S) >= 65);
    assert!(share_percent(&run, modem(2), 2, RIDE_END_S) <= 20);
    assert!(share_percent(&run, modem(3), 2, RIDE_END_S) <= 15);
}

#[test]
fn falling_back_on_a_starved_link_floods_it_and_drops_video() {
    let Some(run) = load("real-ride") else { return };
    let cut_s = mark_s(&run, "cut link 2");
    let flood = data_packets(&run, modem(3), cut_s + 1, cut_s + 4) / 3;
    assert!(
        flood > LINK_3_PACKETS_PER_SECOND * 3 / 2,
        "{flood} packets/s into a 2 Mbit/s link"
    );
    assert!(run.srt.dropped_packets > 0, "the viewer loses video");
}
