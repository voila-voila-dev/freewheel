//! Random traffic, hostile or not: no panic, and the limits always hold.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::net::{Ipv4Addr, SocketAddrV4};

use bonding_protocol::srtla::{GroupId, PacketType};
use bonding_receiver::srtla::{
    GroupHandle, MAX_GROUPS, MAX_LINKS_PER_GROUP, Receiver, ReceiverEvent,
};
use support::{FakeIo, answer, data, id_in, reg1, reg2, srt_control};

const OPERATIONS: usize = 100_000;

/// Small deterministic generator (xorshift64): same seed, same run, on every machine.
struct Random(u64);

impl Random {
    fn below(&mut self, bound: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        usize::try_from(x % bound as u64).unwrap_or_default()
    }
}

fn address(random: &mut Random) -> SocketAddrV4 {
    let ip = u8::try_from(random.below(3)).unwrap_or_default();
    let port = u16::try_from(random.below(400)).unwrap_or_default();
    SocketAddrV4::new(Ipv4Addr::new(10, ip, 0, 2), 1_000 + port)
}

#[test]
fn limits_hold_under_random_traffic() {
    let mut random = Random(0x5EED_CAFE);
    let (mut receiver, mut io) = (Receiver::new(), FakeIo::default());
    let mut ids: Vec<GroupId> = Vec::new();
    let mut live: BTreeSet<GroupHandle> = BTreeSet::new();
    let mut now_s = 0;
    for _ in 0..OPERATIONS {
        let from = address(&mut random);
        match random.below(10) {
            0 => receiver.handle_link_datagram(now_s, from, &reg1(1), &mut io),
            1 | 2 => {
                let id = ids
                    .get(random.below(ids.len() + 1))
                    .copied()
                    .unwrap_or([3; 256]);
                receiver.handle_link_datagram(now_s, from, &reg2(&id), &mut io);
            }
            3 | 4 => {
                let number = u32::try_from(random.below(1 << 20)).unwrap_or_default();
                receiver.handle_link_datagram(now_s, from, &data(number), &mut io);
            }
            5 => {
                receiver.handle_link_datagram(now_s, from, &answer(PacketType::Keepalive), &mut io);
            }
            6 | 7 => {
                if let Some(group) = live.iter().nth(random.below(live.len().max(1))).copied() {
                    let kind = [PacketType::SrtAck, PacketType::SrtNak][random.below(2)];
                    let packet = srt_control(kind);
                    let len = if random.below(50) == 0 {
                        8
                    } else {
                        packet.len()
                    };
                    receiver.handle_srt_datagram(group, &packet[..len], &mut io);
                }
            }
            8 => {
                now_s += u64::try_from(random.below(3)).unwrap_or_default();
                receiver.housekeeping(now_s, &mut io);
            }
            _ => {
                let len = random.below(40);
                let bytes: Vec<u8> = (0..len)
                    .map(|_| u8::try_from(random.below(256)).unwrap_or(0))
                    .collect();
                receiver.handle_link_datagram(now_s, from, &bytes, &mut io);
            }
        }
        for sent in io.take_sent() {
            if let support::Sent::ToLink(_, bytes) = sent
                && let Some(id) = id_in(&bytes)
            {
                ids.push(id);
            }
        }
        for event in io.events.drain(..) {
            match event {
                ReceiverEvent::GroupRegistered { group, .. } => {
                    live.insert(group);
                }
                ReceiverEvent::GroupRemoved { group, .. } => {
                    live.remove(&group);
                }
                _ => {}
            }
        }
        assert!(receiver.group_count() <= MAX_GROUPS);
        assert_eq!(receiver.group_count(), live.len());
        let mut owners: BTreeMap<SocketAddrV4, GroupHandle> = BTreeMap::new();
        for group in &live {
            assert!(receiver.links(*group).count() <= MAX_LINKS_PER_GROUP);
            for link in receiver.links(*group) {
                assert!(
                    owners.insert(link, *group).is_none(),
                    "{link} in two groups"
                );
                assert_eq!(receiver.group_of(link), Some(*group));
            }
        }
    }
    assert!(!ids.is_empty(), "the run registered groups");
}
