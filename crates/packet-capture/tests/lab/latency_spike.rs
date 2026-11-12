//! `latency-spike`: after a 5 s latency spike, link 2 never gets its share back.

use crate::support::{load, mark_s, modem, share_percent};

#[test]
fn a_link_never_recovers_from_a_latency_spike() {
    let Some(run) = load("latency-spike") else {
        return;
    };
    let spike_s = mark_s(&run, "latency spike");
    assert!(share_percent(&run, modem(2), 2, spike_s) >= 40);
    assert!(share_percent(&run, modem(2), spike_s + 10, spike_s + 18) <= 15);
}
