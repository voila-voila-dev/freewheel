//! The Noise `IK` handshake: one round trip opens a session between a box and a server that
//! know each other, and nothing else does.

#![allow(clippy::unwrap_used)]

mod support;

use bonding_protocol::freewheel::{HELLO_MESSAGE_LEN, WELCOME_MESSAGE_LEN};
use freewheel_noise::{AuthorizedKeys, Cipher, HandshakeError, Initiator, KeyPair, read_hello};
use support::{SESSION, data, open, open_datagram, pair, seal};

const CIPHERS: [Cipher; 2] = [Cipher::ChaCha20Poly1305, Cipher::Aes256Gcm];

#[test]
fn a_known_box_opens_a_session_both_ways_with_either_cipher() {
    for cipher in CIPHERS {
        let pair = pair(cipher);
        let (mut client, mut server) = open(&pair);
        let up = data(b"from the box");
        assert_eq!(open_datagram(&mut server, &seal(&mut client, &up)), Ok(up));
        let down = data(b"from the server");
        assert_eq!(
            open_datagram(&mut client, &seal(&mut server, &down)),
            Ok(down)
        );
    }
}

#[test]
fn the_hello_names_the_box_and_its_number() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let initiator = Initiator::start(&pair.box_keys, SESSION, 42).unwrap();
    assert_eq!(initiator.hello_message().len(), HELLO_MESSAGE_LEN);
    let hello = read_hello(&pair.server_keys, SESSION, initiator.hello_message()).unwrap();
    assert_eq!(hello.box_key(), pair.box_pair.public);
    assert_eq!(hello.handshake_number(), 42);
    assert_eq!(hello.ephemeral()[..], initiator.hello_message()[..32]);
}

#[test]
fn a_box_not_on_the_list_is_refused() {
    let mut pair = pair(Cipher::ChaCha20Poly1305);
    let other = KeyPair::generate().unwrap();
    pair.server_keys.authorized = AuthorizedKeys::new(vec![other.public]);
    let initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    assert_eq!(
        read_hello(&pair.server_keys, SESSION, initiator.hello_message()).map(|_| ()),
        Err(HandshakeError::UnknownBox(pair.box_pair.public))
    );
}

#[test]
fn a_box_with_the_wrong_server_key_gets_nowhere() {
    let mut pair = pair(Cipher::ChaCha20Poly1305);
    pair.box_keys.server = KeyPair::generate().unwrap().public;
    let initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    assert_eq!(
        read_hello(&pair.server_keys, SESSION, initiator.hello_message()).map(|_| ()),
        Err(HandshakeError::Failed)
    );
}

#[test]
fn a_hello_replayed_under_another_session_does_not_authenticate() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    assert_eq!(
        read_hello(&pair.server_keys, SESSION + 1, initiator.hello_message()).map(|_| ()),
        Err(HandshakeError::Failed)
    );
}

#[test]
fn both_sides_must_use_the_same_cipher() {
    let mut pair = pair(Cipher::ChaCha20Poly1305);
    pair.server_keys.cipher = Cipher::Aes256Gcm;
    let initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    assert!(read_hello(&pair.server_keys, SESSION, initiator.hello_message()).is_err());
}

#[test]
fn an_altered_hello_does_not_authenticate() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    for at in 0..HELLO_MESSAGE_LEN {
        let mut hello = *initiator.hello_message();
        hello[at] ^= 0x01;
        assert!(
            read_hello(&pair.server_keys, SESSION, &hello).is_err(),
            "byte {at}"
        );
    }
}

#[test]
fn a_forged_welcome_changes_nothing_and_the_real_one_still_opens() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let mut initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    let hello = read_hello(&pair.server_keys, SESSION, initiator.hello_message()).unwrap();
    let mut welcome = [0u8; WELCOME_MESSAGE_LEN];
    let mut server = hello.welcome(&pair.server_keys, &mut welcome).unwrap();
    let mut forged = welcome;
    forged[40] ^= 0x80;
    assert_eq!(
        initiator.read_welcome(&forged).map(|_| ()),
        Err(HandshakeError::Failed)
    );
    assert_eq!(
        initiator.read_welcome(&welcome[..10]).map(|_| ()),
        Err(HandshakeError::WrongLength { len: 10 })
    );
    let mut client = initiator.read_welcome(&welcome).unwrap();
    let up = data(b"after a forged welcome");
    assert_eq!(open_datagram(&mut server, &seal(&mut client, &up)), Ok(up));
}

#[test]
fn a_welcome_for_another_hello_does_not_open() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let first = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    let mut second = Initiator::start(&pair.box_keys, SESSION, 2).unwrap();
    let hello = read_hello(&pair.server_keys, SESSION, first.hello_message()).unwrap();
    let mut welcome = [0u8; WELCOME_MESSAGE_LEN];
    hello.welcome(&pair.server_keys, &mut welcome).unwrap();
    assert!(second.read_welcome(&welcome).is_err());
}

#[test]
fn every_hello_is_new() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let first = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    let second = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    assert_ne!(first.hello_message(), second.hello_message());
}

#[test]
fn two_sessions_have_unrelated_keys() {
    let pair = pair(Cipher::ChaCha20Poly1305);
    let (mut first_client, _) = open(&pair);
    let (_, mut second_server) = open(&pair);
    let up = data(b"for the first session");
    assert!(open_datagram(&mut second_server, &seal(&mut first_client, &up)).is_err());
}
