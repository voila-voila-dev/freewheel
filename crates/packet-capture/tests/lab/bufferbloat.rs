//! `bufferbloat`: a 2 Mbit/s link with a deep buffer gets far less than it could carry.

use crate::support::{load, modem, share_percent};

#[test]
fn a_bloated_link_is_underused() {
    let Some(run) = load("bufferbloat") else {
        return;
    };
    assert!(share_percent(&run, modem(2), 5, 28) <= 30);
    assert_eq!(run.srt.dropped_packets, 0);
}
