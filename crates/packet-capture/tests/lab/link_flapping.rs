//! `link-flapping`: a link that drops 1 s every 3 s is all but abandoned.

use crate::support::{load, modem, share_percent};

#[test]
fn a_flapping_link_is_starved() {
    let Some(run) = load("link-flapping") else {
        return;
    };
    assert!(share_percent(&run, modem(2), 1, 5) >= 40);
    assert!(share_percent(&run, modem(2), 15, 30) <= 10);
    assert_eq!(run.srt.dropped_packets, 0);
}
