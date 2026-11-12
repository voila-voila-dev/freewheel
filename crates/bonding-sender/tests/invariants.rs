//! 100 000 random operations: windows stay within bounds, nothing panics, the list stays sane.

mod support;

use bonding_protocol::srtla::PacketType;
use bonding_sender::srtla::{WINDOW_MAX_MILLIPACKETS, WINDOW_MIN_MILLIPACKETS};
use support::{
    CALLER, FakeIo, TestResult, answer, data, full_id, registration, sender_on, source, srt_ack,
    srt_nak, srtla_ack,
};

/// xorshift64: deterministic, no dependency.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

#[test]
fn random_operations_keep_windows_in_bounds() -> TestResult {
    let mut random = Random(0x5EED_F00D);
    let mut io = FakeIo::default();
    let mut sender = sender_on(&[1, 2, 3], &mut io)?;
    let mut now_ms = 0;
    let mut next_number = 1_000_u32;
    for _ in 0..100_000 {
        now_ms += random.below(50);
        let links: Vec<_> = sender.links(now_ms).map(|status| status.handle).collect();
        let Some(&on) = links.get(usize::try_from(random.below(links.len().max(1) as u64))?) else {
            sender.reload_sources(&[source(1), source(2)], &mut io);
            continue;
        };
        let near = next_number.saturating_sub(u32::try_from(random.below(300))?);
        match random.below(14) {
            0..=4 => {
                sender.handle_srt_datagram(now_ms, CALLER, &data(next_number), &mut io);
                next_number += 1;
            }
            5 => sender.handle_link_datagram(
                now_ms,
                on,
                &srtla_ack(&[near, near + 1, near + 2]),
                &mut io,
            ),
            6 => sender.handle_link_datagram(now_ms, on, &srt_ack(near), &mut io),
            7 => sender.handle_link_datagram(
                now_ms,
                on,
                &srt_nak(&[0x8000_0000 | near, near + 5]),
                &mut io,
            ),
            8 => sender.handle_link_datagram(now_ms, on, &answer(PacketType::Reg3), &mut io),
            9 => sender.handle_link_datagram(now_ms, on, &answer(PacketType::RegNgp), &mut io),
            10 => sender.handle_link_datagram(
                now_ms,
                on,
                &registration(PacketType::Reg2, &full_id()),
                &mut io,
            ),
            11 => sender.handle_link_datagram(now_ms, on, &answer(PacketType::Keepalive), &mut io),
            12 => {
                let count = random.below(4);
                let sources: Vec<_> = (1..=4)
                    .filter(|k| u64::from(*k) <= count + 1)
                    .map(source)
                    .collect();
                if random.below(50) == 0 {
                    sender.reload_sources(&sources, &mut io);
                }
            }
            _ => {
                let garbage: Vec<u8> = (0..random.below(40))
                    .map(|_| u8::try_from(random.below(256)).unwrap_or(0))
                    .collect();
                sender.handle_link_datagram(now_ms, on, &garbage, &mut io);
            }
        }
        sender.housekeeping(now_ms, &mut io);
        io.sent.clear();
        io.events.clear();
        for status in sender.links(now_ms) {
            assert!(status.window_millipackets >= WINDOW_MIN_MILLIPACKETS);
            assert!(status.window_millipackets <= WINDOW_MAX_MILLIPACKETS + 29);
        }
        let sources: Vec<_> = sender.links(now_ms).map(|status| status.source).collect();
        let mut unique = sources.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), sources.len(), "one link per address");
    }
    Ok(())
}
