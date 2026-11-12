//! `sender-restart`: `srtla_send` restarts with a new id and new ports.

use crate::support::{links_of, load, modem};

#[test]
fn a_restarted_sender_gets_a_new_group() {
    let Some(run) = load("sender-restart") else {
        return;
    };
    assert_eq!(run.server_side.group_ids.len(), 2);
    for k in [1, 2] {
        assert_eq!(
            links_of(&run.box_side, modem(k)).count(),
            2,
            "link {k}: one port per run"
        );
    }
}
