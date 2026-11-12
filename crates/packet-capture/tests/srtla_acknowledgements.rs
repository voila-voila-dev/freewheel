//! How upstream SRTLA acknowledges, checked on a real capture.

mod support;

use bonding_protocol::srtla::PacketType;
use support::report_of;

const FIXTURE: &[u8] = include_bytes!("fixtures/steady-start.pcap");
const SEQUENCE_NUMBERS_PER_SRTLA_ACK: usize = 10;

#[test]
fn srtla_acks_carry_ten_sequence_numbers_from_the_same_link() {
    for stats in report_of(FIXTURE).links.values() {
        assert!(
            stats
                .srtla_ack_sizes
                .keys()
                .all(|&size| size == SEQUENCE_NUMBERS_PER_SRTLA_ACK)
        );
        assert!(stats.srtla_acked > 0);
        assert_eq!(stats.srtla_acked, stats.srtla_acked_on_same_link);
    }
}

#[test]
fn srt_acks_are_sent_on_every_link() {
    let report = report_of(FIXTURE);
    let srt_acks: Vec<Option<&u64>> = report
        .links
        .values()
        .map(|stats| stats.downlink_control.get(&(PacketType::SrtAck as u16)))
        .collect();
    assert!(srt_acks.iter().all(Option::is_some));
    assert!(srt_acks.windows(2).all(|pair| pair[0] == pair[1]));
}
