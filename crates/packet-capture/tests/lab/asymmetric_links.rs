//! `asymmetric-links`: upstream starves slow links far below their capacity.

use crate::support::{load, modem, share_percent};

#[test]
fn the_fast_link_takes_almost_everything() {
    let Some(run) = load("asymmetric-links") else {
        return;
    };
    // Link 2 could carry 2 of the 3 Mbit/s (≈ 40 %), yet gets almost nothing.
    assert!(share_percent(&run, modem(1), 5, 28) >= 90);
    assert!(share_percent(&run, modem(2), 5, 28) <= 5);
    assert_eq!(run.srt.dropped_packets, 0);
}
