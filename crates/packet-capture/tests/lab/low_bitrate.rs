//! `low-bitrate`: at 300 kbit/s, ACK batches stay at 10 and links stay registered.

use crate::support::{load, modem, share_percent};

#[test]
fn a_slow_stream_keeps_both_links_and_ten_number_acks() {
    let Some(run) = load("low-bitrate") else {
        return;
    };
    for k in [1, 2] {
        assert!(share_percent(&run, modem(k), 2, 28) >= 25, "link {k}");
    }
    for stats in run.box_side.links.values() {
        assert!(stats.srtla_ack_sizes.keys().all(|&size| size == 10));
    }
    assert_eq!(run.srt.dropped_packets, 0);
}
