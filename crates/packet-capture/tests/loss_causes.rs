//! Why lost frames were lost: the emulated network read back from the marks, then each lost
//! frame against it.

use std::collections::HashSet;
use std::fmt::Write as _;

use packet_capture::{
    EmulatedUplinks, FrameDigests, FrameTime, LossCauses, LossEvidence, Mark, SentPacket,
    UplinkSetting,
};

const SECOND_US: u64 = 1_000_000;
const START_US: u64 = 1_000 * SECOND_US;
const FRAME_US: u64 = 33_333;
/// 30 frames a second of this size: 3 Mbit/s of video, 3.5 on the wire.
const FRAME_BYTES: u64 = 12_500;
const FRAMES: u64 = 300;

fn marks(lines: &[(u64, &str)]) -> Vec<Mark> {
    lines
        .iter()
        .map(|(at_s, label)| Mark {
            at_us: START_US + at_s * SECOND_US,
            label: (*label).to_owned(),
        })
        .collect()
}

#[test]
fn reads_rate_loss_and_cuts() {
    assert_eq!(
        UplinkSetting::parse("delay 30ms 10ms rate 6mbit limit 100 loss 0.5%"),
        UplinkSetting {
            rate_bps: Some(6_000_000),
            dead: false,
            lossy: true
        }
    );
    assert_eq!(
        UplinkSetting::parse("delay 40ms rate 2400kbit limit 100").rate_bps,
        Some(2_400_000)
    );
    assert!(UplinkSetting::parse("loss 100%").dead);
    assert!(UplinkSetting::parse("delay 20ms loss gemodel 1% 10% 70% 0.1%").lossy);
    assert_eq!(UplinkSetting::parse("delay 20ms"), UplinkSetting::default());
    assert!(
        UplinkSetting::parse("delay 20ms")
            .capacity_bps()
            .is_infinite()
    );
}

#[test]
fn only_uplinks_count_and_settings_follow_time() {
    let uplinks = EmulatedUplinks::from_marks(&marks(&[
        (0, "netem link 1 both: delay 20ms rate 4mbit"),
        (0, "netem link 2 up: rate 2mbit"),
        (0, "netem link 2 down: loss 100%"),
        (1, "cut link 1 both"),
        (2, "netem link 1 up: loss 100%"),
    ]));
    assert!(!uplinks.is_empty());
    assert!(!uplinks.setting_at(2, START_US + SECOND_US).dead);
    assert_eq!(
        uplinks.setting_at(1, START_US + SECOND_US).rate_bps,
        Some(4_000_000)
    );
    assert!(uplinks.setting_at(1, START_US + 3 * SECOND_US).dead);
    // Before its first mark a link is not limited.
    assert!(uplinks.setting_at(3, START_US).capacity_bps().is_infinite());
    // Second 0–1: 6 Mbit/s; 1.5–2.5: link 1 dies halfway, 2 + 4 / 2.
    let whole = uplinks.capacity_bps_between(START_US, START_US + SECOND_US);
    assert!((whole - 6_000_000.0).abs() < 1.0, "{whole}");
    let half =
        uplinks.capacity_bps_between(START_US + 3 * SECOND_US / 2, START_US + 5 * SECOND_US / 2);
    assert!((half - 4_000_000.0).abs() < 1.0, "{half}");
}

#[test]
fn a_run_without_netem_marks_has_no_uplinks() {
    assert!(EmulatedUplinks::from_marks(&marks(&[(0, "cut link 1 both")])).is_empty());
}

/// Every frame sent; `lost` frames (by index) arrive damaged.
fn listings(lost: &HashSet<u64>) -> (FrameDigests, FrameDigests, Vec<FrameTime>) {
    let header = "#tb 0: 1/90000\n";
    let (mut sent, mut received) = (header.to_owned(), header.to_owned());
    let mut times = Vec::new();
    for index in 0..FRAMES {
        let pts = i64::try_from(index * 3_000).unwrap_or_default();
        let _ = writeln!(sent, "0, {pts}, {pts}, 3000, {FRAME_BYTES}, h{index}");
        let hash = if lost.contains(&index) { "bad" } else { "" };
        let _ = writeln!(
            received,
            "0, {pts}, {pts}, 3000, {FRAME_BYTES}, h{index}{hash}"
        );
        times.push(FrameTime {
            pts,
            at_us: START_US + index * FRAME_US,
        });
    }
    (
        FrameDigests::parse(&sent),
        FrameDigests::parse(&received),
        times,
    )
}

/// One packet per frame, on `link`; those of `undelivered` frames never reach the server.
fn packets(link: u8, undelivered: &HashSet<u64>) -> (Vec<SentPacket>, HashSet<u32>) {
    let sent: Vec<SentPacket> = (0..FRAMES)
        .map(|index| SentPacket {
            at_us: START_US + index * FRAME_US,
            link,
            sequence: u32::try_from(index).unwrap_or_default(),
        })
        .collect();
    let delivered = sent
        .iter()
        .filter(|packet| !undelivered.contains(&u64::from(packet.sequence)))
        .map(|packet| packet.sequence)
        .collect();
    (sent, delivered)
}

fn classify(netem: &[(u64, &str)], lost: &HashSet<u64>, undelivered: &HashSet<u64>) -> LossCauses {
    let uplinks = EmulatedUplinks::from_marks(&marks(netem));
    let (sent, received, times) = listings(lost);
    let (box_packets, delivered) = packets(1, undelivered);
    LossCauses::classify(&LossEvidence {
        uplinks: &uplinks,
        sent: &sent,
        received: &received,
        sent_times: &times,
        box_packets: &box_packets,
        delivered: &delivered,
    })
}

fn frames_in_second(second: u64) -> HashSet<u64> {
    (second * 30..second * 30 + 30).collect()
}

#[test]
fn frames_lost_while_the_links_carry_less_than_the_stream_are_capacity() {
    let causes = classify(
        &[
            (0, "netem link 1 up: rate 8mbit"),
            (4, "netem link 1 up: rate 1mbit"),
            (6, "netem link 1 up: rate 8mbit"),
        ],
        &frames_in_second(5),
        &HashSet::new(),
    );
    assert_eq!(causes.short_seconds, 2);
    assert_eq!(causes.capacity, 30);
    assert_eq!(causes.total(), 30);
}

#[test]
fn frames_lost_on_a_lossy_link_with_capacity_to_spare_are_isolated_losses() {
    let lost: HashSet<u64> = [40, 130, 220].into();
    let causes = classify(&[(0, "netem link 1 up: rate 8mbit loss 2%")], &lost, &lost);
    assert_eq!(causes.short_seconds, 0);
    assert_eq!(causes.isolated_loss, 3);
}

#[test]
fn packets_sent_into_a_dead_link_are_a_link_down() {
    let lost: HashSet<u64> = [100].into();
    let causes = classify(
        &[
            (0, "netem link 1 up: rate 8mbit loss 1%"),
            (0, "netem link 2 up: rate 8mbit"),
            (3, "netem link 1 up: loss 100%"),
        ],
        &lost,
        &lost,
    );
    assert_eq!(causes.link_down, 1);
}

#[test]
fn a_loss_on_a_link_that_drops_nothing_at_random_is_its_queue() {
    let lost: HashSet<u64> = [100].into();
    let causes = classify(&[(0, "netem link 1 up: rate 8mbit limit 50")], &lost, &lost);
    assert_eq!(causes.queue, 1);
}

#[test]
fn frames_lost_after_a_hole_with_nothing_lost_on_the_way_are_recovery_or_late() {
    let lost: HashSet<u64> = [8 * 30 + 3, 9 * 30 + 3].into_iter().collect();
    let causes = classify(
        &[
            (0, "netem link 1 up: rate 8mbit"),
            (2, "netem link 1 up: loss 100%"),
            (3, "netem link 1 up: rate 8mbit"),
        ],
        &lost,
        &HashSet::new(),
    );
    // Second 2 was short: 8 s and 9 s are past its recovery (2–5 s after): both late.
    assert_eq!((causes.recovery, causes.late), (0, 2));
    let lost: HashSet<u64> = [5 * 30].into();
    let causes = classify(
        &[
            (0, "netem link 1 up: rate 8mbit"),
            (2, "netem link 1 up: loss 100%"),
            (3, "netem link 1 up: rate 8mbit"),
        ],
        &lost,
        &HashSet::new(),
    );
    assert_eq!(causes.recovery, 1);
}

#[test]
fn damaged_frames_are_those_frame_check_counts() {
    let lost: HashSet<u64> = [3, 4, 200].into();
    let (sent, received, _) = listings(&lost);
    let damaged: Vec<i64> = sent.damaged_in(&received).iter().map(|f| f.pts).collect();
    assert_eq!(damaged, vec![9_000, 12_000, 600_000]);
    assert_eq!(
        packet_capture::FrameCheck::compare(&sent, &received).damaged(),
        3
    );
    assert_eq!(sent.frames[0].size_bytes, FRAME_BYTES);
}
