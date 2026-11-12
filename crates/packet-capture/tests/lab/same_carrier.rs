//! `same-carrier`: two SIMs of one operator share a public address; upstream tells them apart
//! by port and splits evenly.

use std::net::Ipv4Addr;

use bonding_protocol::srtla::PacketType;

use crate::support::{links_of, load, modem, received, share_percent};

const SHARED_PUBLIC_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 100, 1, 1);

#[test]
fn two_links_behind_one_public_address_both_work() {
    let Some(run) = load("same-carrier") else {
        return;
    };
    assert_eq!(
        links_of(&run.server_side, SHARED_PUBLIC_ADDRESS).count(),
        2,
        "one public address, two ports"
    );
    for k in [1, 2] {
        assert_eq!(received(&run, modem(k), PacketType::Reg3 as u16), 1);
        assert!(share_percent(&run, modem(k), 2, 28) >= 40, "link {k}");
    }
    assert_eq!(run.srt.dropped_packets, 0);
}
