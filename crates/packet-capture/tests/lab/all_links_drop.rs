//! `all-links-drop`: a 6 s tunnel; everything comes back without restarting.

use crate::support::{data_packets, load, mark_s, modem};

#[test]
fn a_short_tunnel_is_survived() {
    let Some(run) = load("all-links-drop") else {
        return;
    };
    let restore_s = mark_s(&run, "restore link 2");
    for k in [1, 2] {
        assert!(
            data_packets(&run, modem(k), restore_s + 5, restore_s + 15) > 0,
            "link {k}"
        );
    }
    assert!(
        run.sender_exits().iter().all(|(_, code)| *code != 1),
        "sender must not give up"
    );
}
