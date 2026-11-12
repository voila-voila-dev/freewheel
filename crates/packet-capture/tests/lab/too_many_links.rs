//! `too-many-links`: nine links, eight allowed; the ninth is refused but still used.

use bonding_protocol::srtla::PacketType;

use crate::support::{load, modem, received};

#[test]
fn the_ninth_link_is_refused() {
    let Some(run) = load("too-many-links") else {
        return;
    };
    let registered = (1..=9)
        .filter(|&k| received(&run, modem(k), PacketType::Reg3 as u16) > 0)
        .count();
    let refused: Vec<u8> = (1..=9)
        .filter(|&k| received(&run, modem(k), PacketType::RegErr as u16) > 0)
        .collect();
    assert_eq!(registered, 8);
    assert_eq!(refused.len(), 1);
    // REG_ERR counts as "heard from the server", so the refused link is used for data
    // that the receiver throws away, until it times out.
    let refused_link = modem(refused[0]);
    assert!(crate::support::data_packets(&run, refused_link, 0, 3) > 0);
    assert_eq!(run.srt.dropped_packets, 0);
}
