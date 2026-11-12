//! `jitter-reordering`: upstream treats reordering as loss and abandons the jittery link.

use crate::support::{load, modem, share_percent};

#[test]
fn a_jittery_link_is_abandoned() {
    let Some(run) = load("jitter-reordering") else {
        return;
    };
    // Nothing is lost on link 2, only reordered, yet it ends up nearly unused.
    assert!(share_percent(&run, modem(2), 10, 28) <= 5);
    assert_eq!(run.srt.dropped_packets, 0);
}
