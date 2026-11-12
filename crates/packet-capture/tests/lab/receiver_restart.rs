//! `receiver-restart`: `srtla_rec` restarts; the box keeps sending into the void, then re-registers.

use bonding_protocol::srtla::PacketType;

use crate::support::{data_packets, load, mark_s, modem, received};

#[test]
fn a_restarted_receiver_means_a_new_group_after_seconds_of_loss() {
    let Some(run) = load("receiver-restart") else {
        return;
    };
    assert_eq!(run.server_side.group_ids.len(), 2);
    let stop_s = mark_s(&run, "receiver stopped");
    // The new receiver drops the old group's packets silently; the box notices only after 4 s.
    assert!(data_packets(&run, modem(1), stop_s + 1, stop_s + 3) > 0);
    assert!(received(&run, modem(1), PacketType::RegNgp as u16) >= 2);
}
