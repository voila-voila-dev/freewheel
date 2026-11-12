//! `random-loss`: 5 % loss on link 2 shrinks its share; SRT repairs the losses.

use crate::support::{load, modem, share_percent};

#[test]
fn a_lossy_link_loses_share_but_no_video_is_lost() {
    let Some(run) = load("random-loss") else {
        return;
    };
    assert!(share_percent(&run, modem(2), 20, 28) <= 40);
    assert_eq!(run.srt.dropped_packets, 0);
}
