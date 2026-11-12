//! `cold-start`: the receiver starts late; the box drops video until it registers.

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn video_before_registration_is_dropped_by_the_box() {
    let Some(run) = load("cold-start") else {
        return;
    };
    let receiver_s = mark_s(&run, "receiver started");
    for k in [1, 2] {
        assert_eq!(data_packets(&run, modem(k), 0, receiver_s), 0, "link {k}");
        assert!(
            data_packets(&run, modem(k), receiver_s + 2, receiver_s + 10) > 0,
            "link {k}"
        );
    }
}
