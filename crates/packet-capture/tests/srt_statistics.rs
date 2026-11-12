//! Reading the SRT listener's statistics, across restarts.

use packet_capture::SrtStatistics;

#[test]
fn totals_are_summed_across_listener_restarts() {
    let text = [
        r#"{"recv":{"packets":100,"packetsUnique":100,"packetsLost":5,"packetsDropped":1,"packetsRetransmitted":4}}"#,
        r#"{"recv":{"packets":300,"packetsUnique":300,"packetsLost":9,"packetsDropped":2,"packetsRetransmitted":7}}"#,
        "not json",
        r#"{"recv":{"packets":50,"packetsLost":0,"packetsDropped":0,"packetsRetransmitted":0}}"#,
    ]
    .join("\n");
    let statistics = SrtStatistics::parse(&text);
    assert_eq!(statistics.received_packets, 350);
    assert_eq!(statistics.lost_packets, 9);
    assert_eq!(statistics.dropped_packets, 2);
    assert_eq!(statistics.retransmitted_packets, 7);
    assert_eq!(statistics.dropped_ppm(), 2 * 1_000_000 / 352);
}
