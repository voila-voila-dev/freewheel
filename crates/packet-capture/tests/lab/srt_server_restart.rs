//! `srt-server-restart`: the SRT listener restarts; `srtla_rec` destroys the group.

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn losing_the_srt_listener_destroys_the_group() {
    let Some(run) = load("srt-server-restart") else {
        return;
    };
    assert_eq!(run.server_side.group_ids.len(), 2);
    let stop_s = mark_s(&run, "SRT listener stopped");
    assert_eq!(
        data_packets(&run, modem(1), stop_s + 1, stop_s + 3),
        0,
        "stream broken"
    );
    assert!(
        data_packets(&run, modem(1), stop_s + 8, stop_s + 20) > 0,
        "and back"
    );
}
