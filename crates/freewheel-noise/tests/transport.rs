//! Sealed packets: the header and the nonce authenticated, the rest encrypted, each nonce
//! opened once.

#![allow(clippy::unwrap_used)]

mod support;

use bonding_protocol::freewheel::{
    Envelope, MAX_DATAGRAM_LEN, MAX_SEALED_DATA_PAYLOAD_LEN, Packet, SEALED_OVERHEAD_LEN,
};
use freewheel_noise::{Cipher, DEFAULT_REPLAY_WINDOW_PACKETS, OpenError, SealError};
use support::{data, open, open_datagram, pair, seal};

#[test]
fn a_sealed_packet_hides_what_follows_the_header() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut client, _) = open(&pair);
    let payload = [0x55u8; 200];
    let plain = data(&payload);
    let sealed = seal(&mut client, &plain);
    assert_eq!(sealed.len(), plain.len() + SEALED_OVERHEAD_LEN);
    assert_eq!(sealed[0], 0xF1);
    assert_eq!(sealed[1..12], plain[1..12]);
    assert_eq!(sealed[12..20], 0u64.to_be_bytes());
    assert_ne!(sealed[20..20 + 8 + 200], plain[12..]);
    let second = seal(&mut client, &plain);
    assert_eq!(second[12..20], 1u64.to_be_bytes());
    assert_ne!(second[20..], sealed[20..]);
}

#[test]
fn any_altered_byte_is_refused() {
    for cipher in [Cipher::ChaCha20Poly1305, Cipher::Aes256Gcm] {
        let pair = pair(cipher);
        let (mut client, mut server) = open(&pair);
        let sealed = seal(&mut client, &data(&[1u8; 64]));
        for at in 1..sealed.len() {
            let mut altered = sealed.clone();
            altered[at] ^= 0x04;
            let Ok(Envelope::Sealed(read)) = Envelope::decode(&altered) else {
                continue;
            };
            assert_eq!(
                support::open_sealed(&mut server, &read),
                Err(OpenError::Forged),
                "{cipher:?}, byte {at}"
            );
        }
        assert!(open_datagram(&mut server, &sealed).is_ok());
    }
}

#[test]
fn a_header_moved_to_another_link_or_class_is_refused() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut client, mut server) = open(&pair);
    let sealed = seal(&mut client, &data(&[2u8; 64]));
    let mut other_link = sealed.clone();
    other_link[2] = 9;
    assert_eq!(
        open_datagram(&mut server, &other_link),
        Err(OpenError::Forged)
    );
    let mut other_class = sealed.clone();
    other_class[3] = 0x03;
    assert_eq!(
        open_datagram(&mut server, &other_class),
        Err(OpenError::Forged)
    );
    assert!(open_datagram(&mut server, &sealed).is_ok());
}

#[test]
fn a_replayed_packet_is_refused() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut client, mut server) = open(&pair);
    let sealed = seal(&mut client, &data(b"once"));
    assert!(open_datagram(&mut server, &sealed).is_ok());
    assert_eq!(
        open_datagram(&mut server, &sealed),
        Err(OpenError::Replayed)
    );
}

#[test]
fn reordering_within_the_window_is_fine_and_below_it_is_a_replay() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut client, mut server) = open(&pair);
    let plain = data(b"x");
    let packets: Vec<Vec<u8>> = (0..DEFAULT_REPLAY_WINDOW_PACKETS + 10)
        .map(|_| seal(&mut client, &plain))
        .collect();
    // Links reorder: the latest of the window first, then the rest backwards.
    let last_in_window = DEFAULT_REPLAY_WINDOW_PACKETS - 1;
    for index in (0..=last_in_window).rev() {
        assert!(
            open_datagram(&mut server, &packets[index]).is_ok(),
            "{index}"
        );
    }
    assert!(open_datagram(&mut server, &packets[last_in_window + 5]).is_ok());
    // The window slid by 6: nonces 0..=5 are below it, refused whether opened or not.
    assert_eq!(
        open_datagram(&mut server, &packets[3]),
        Err(OpenError::Replayed)
    );
    for (index, packet) in packets
        .iter()
        .enumerate()
        .take(last_in_window + 5)
        .skip(last_in_window + 1)
    {
        assert!(open_datagram(&mut server, packet).is_ok(), "{index}");
    }
}

#[test]
fn a_failed_open_does_not_use_up_the_nonce() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut client, mut server) = open(&pair);
    let sealed = seal(&mut client, &data(b"real"));
    let mut forged = sealed.clone();
    let last = forged.len() - 1;
    forged[last] ^= 1;
    assert_eq!(open_datagram(&mut server, &forged), Err(OpenError::Forged));
    assert!(open_datagram(&mut server, &sealed).is_ok());
}

#[test]
fn the_packet_inside_decodes_as_v0() {
    let pair = pair(Cipher::Aes256Gcm);
    let (mut client, mut server) = open(&pair);
    let plain = data(&[9u8; MAX_SEALED_DATA_PAYLOAD_LEN]);
    let sealed = seal(&mut client, &plain);
    assert_eq!(sealed.len(), MAX_DATAGRAM_LEN);
    let opened = open_datagram(&mut server, &sealed).unwrap();
    assert_eq!(opened, plain);
    assert!(matches!(Packet::decode(&opened), Ok(Packet::Data(_))));
}

#[test]
fn what_does_not_fit_is_not_sealed() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut client, _) = open(&pair);
    let too_long = data(&[0u8; MAX_SEALED_DATA_PAYLOAD_LEN + 1]);
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    assert!(matches!(
        client.seal(&too_long, &mut out),
        Err(SealError::Frame(_))
    ));
}
