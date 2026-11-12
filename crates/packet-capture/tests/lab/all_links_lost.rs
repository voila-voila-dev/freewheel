//! `all-links-lost`: a 25 s tunnel; upstream `srtla_send` gives up and exits.

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn the_sender_exits_after_a_long_tunnel() {
    let Some(run) = load("all-links-lost") else {
        return;
    };
    assert!(
        run.sender_exits().contains(&(1, 1)),
        "exits: {:?}",
        run.sender_exits()
    );
    let cut_s = mark_s(&run, "cut link 1");
    let exit_s = mark_s(&run, "sender 1 exited");
    assert!(
        (cut_s + 12..=cut_s + 18).contains(&exit_s),
        "exit after {} s",
        exit_s - cut_s
    );
    // Nothing comes back by itself: a supervisor must restart it.
    for k in [1, 2] {
        assert_eq!(data_packets(&run, modem(k), exit_s + 1, exit_s + 30), 0);
    }
}
