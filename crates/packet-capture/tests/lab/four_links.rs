//! `four-links`: four equal links each take about a quarter.

use crate::support::{load, modem, share_percent};

#[test]
fn four_equal_links_each_take_about_a_quarter() {
    let Some(run) = load("four-links") else {
        return;
    };
    for k in 1..=4 {
        let share = share_percent(&run, modem(k), 2, 28);
        assert!((15..=35).contains(&share), "link {k}: {share} %");
    }
    assert_eq!(run.srt.dropped_packets, 0);
}
