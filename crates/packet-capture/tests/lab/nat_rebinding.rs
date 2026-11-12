//! `nat-rebinding`: the carrier NAT gives link 2 a new port; the server sees a new link.

use std::net::Ipv4Addr;

use bonding_protocol::srtla::PacketType;

use crate::support::{links_of, load, modem, received};

const LINK_2_PUBLIC_ADDRESS: Ipv4Addr = Ipv4Addr::new(10, 100, 2, 1);

#[test]
fn a_new_nat_port_means_registering_again() {
    let Some(run) = load("nat-rebinding") else {
        return;
    };
    assert!(
        links_of(&run.server_side, LINK_2_PUBLIC_ADDRESS).count() >= 2,
        "two public ports"
    );
    assert_eq!(received(&run, modem(2), PacketType::Reg3 as u16), 2);
}
