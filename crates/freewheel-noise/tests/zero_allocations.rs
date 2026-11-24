//! Sealing and opening allocate nothing: the count does not grow with the packets.

#![allow(clippy::unwrap_used)]

mod support;

use alloc_counter::{CountingAllocator, count_allocations};
use bonding_protocol::freewheel::{Envelope, MAX_DATAGRAM_LEN};
use freewheel_noise::Cipher;
use support::{data, open, pair};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn round_trips(cipher: Cipher, packets: usize) -> usize {
    let pair = pair(cipher);
    let (mut client, mut server) = open(&pair);
    let plain = data(&[7u8; 1_316]);
    let mut sealed = [0u8; MAX_DATAGRAM_LEN];
    let mut opened = [0u8; MAX_DATAGRAM_LEN];
    let mut bytes = 0;
    for _ in 0..packets {
        let len = client.seal(&plain, &mut sealed).unwrap();
        let Ok(Envelope::Sealed(read)) = Envelope::decode(&sealed[..len]) else {
            continue;
        };
        bytes += server.open(&read, &mut opened).unwrap();
    }
    bytes
}

#[test]
fn sealing_and_opening_allocate_nothing_per_packet() {
    for cipher in [Cipher::ChaCha20Poly1305, Cipher::Aes256Gcm] {
        let (small_bytes, small) = count_allocations(|| round_trips(cipher, 100));
        let (large_bytes, large) = count_allocations(|| round_trips(cipher, 10_000));
        assert!(large_bytes > small_bytes && small_bytes > 0);
        assert_eq!(small, large, "{cipher:?}: {small} for 100, {large} for 10k");
    }
}
