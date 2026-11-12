//! `steady`: two equal links share the traffic and nothing is lost.

use crate::support::{load, modem, share_percent};

#[test]
fn equal_links_share_the_traffic() {
    let Some(run) = load("steady") else { return };
    for k in [1, 2] {
        let share = share_percent(&run, modem(k), 2, 28);
        assert!((35..=65).contains(&share), "link {k}: {share} %");
    }
    assert_eq!(run.srt.dropped_packets, 0);
    assert_eq!(run.server_side.group_ids.len(), 1);
}
