//! `hostile-packets`: malformed and unauthorised packets from a stranger.

use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::srtla::PacketType;

use crate::support::load;

const STRANGER: Ipv4Addr = Ipv4Addr::new(10, 3, 0, 2);

fn answers(run: &packet_capture::LabRun, port: u16) -> Vec<(u16, u64)> {
    run.box_side
        .links
        .get(&SocketAddrV4::new(STRANGER, port))
        .map(|stats| {
            stats
                .downlink_control
                .iter()
                .map(|(kind, count)| (*kind, *count))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn strangers_get_silence_except_for_registration() {
    let Some(run) = load("hostile-packets") else {
        return;
    };
    assert!(answers(&run, 41000).is_empty(), "1-byte packet");
    assert!(answers(&run, 41001).is_empty(), "REG1 one byte short");
    assert_eq!(
        answers(&run, 41002),
        [(PacketType::RegNgp as u16, 1)],
        "unknown group"
    );
    assert!(answers(&run, 41003).is_empty(), "keepalive is not echoed");
    assert!(answers(&run, 41004).is_empty(), "data is dropped");
    assert!(answers(&run, 41005).is_empty(), "REG3 is ignored");
    assert_eq!(
        answers(&run, 41006),
        [(PacketType::Reg2 as u16, 1), (PacketType::RegErr as u16, 1)],
        "a second REG1 from the same address is refused"
    );
    assert_eq!(
        run.server_side.group_ids.len(),
        2,
        "the stranger did create a group"
    );
    assert_eq!(run.srt.dropped_packets, 0, "the real stream is unaffected");
}
