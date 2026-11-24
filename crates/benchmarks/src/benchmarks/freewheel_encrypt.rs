//! Sealing a full datagram on one side and opening it on the other: what encryption adds per
//! packet (M1b step 7), with each cipher. The ROCK 5T's numbers come in M3, where they choose
//! the default (`decisions.md`, 2026-11-10).

use crate::Benchmark;
use bonding_protocol::freewheel::{
    Data, Envelope, Header, MAX_DATAGRAM_LEN, MAX_SEALED_DATA_PAYLOAD_LEN, Packet, TrafficClass,
    WELCOME_MESSAGE_LEN,
};
use freewheel_noise::{
    AuthorizedKeys, BoxKeys, Cipher, Initiator, KeyPair, ServerKeys, Transport, read_hello,
};
use std::hint::black_box;

/// `freewheel_encrypt_chacha20poly1305`: seal then open one full datagram, ChaCha20-Poly1305.
pub const CHACHA20POLY1305: Benchmark = Benchmark {
    name: "freewheel_encrypt_chacha20poly1305",
    run: run_chacha,
};

/// `freewheel_encrypt_aes256gcm`: seal then open one full datagram, AES-256-GCM.
pub const AES256GCM: Benchmark = Benchmark {
    name: "freewheel_encrypt_aes256gcm",
    run: run_aes,
};

fn run_chacha(packets: u64) -> u64 {
    run(Cipher::ChaCha20Poly1305, packets)
}

fn run_aes(packets: u64) -> u64 {
    run(Cipher::Aes256Gcm, packets)
}

/// One handshake (outside the per-packet count: its cost is the same for any number of
/// packets), then `packets` full live datagrams sealed by the box and opened by the server.
fn run(cipher: Cipher, packets: u64) -> u64 {
    let Some((mut box_side, mut server_side)) = open_session(cipher) else {
        return 0;
    };
    let payload = [0x47u8; MAX_SEALED_DATA_PAYLOAD_LEN];
    let mut plain = [0u8; MAX_DATAGRAM_LEN];
    let mut sealed = [0u8; MAX_DATAGRAM_LEN];
    let mut opened = [0u8; MAX_DATAGRAM_LEN];
    let mut checksum = 0u64;
    for sequence in 0..packets {
        let packet = Packet::Data(Data {
            header: Header {
                session_id: 0x5EED,
                link_id: 1,
            },
            class: TrafficClass::Live,
            sequence: u32::try_from(sequence & u64::from(u32::MAX)).unwrap_or(0),
            sent_at_us: 0,
            payload: black_box(&payload),
        });
        let Some(plain) = packet
            .encode(&mut plain)
            .ok()
            .and_then(|len| plain.get(..len))
        else {
            continue;
        };
        let Some(datagram) = box_side
            .seal(plain, &mut sealed)
            .ok()
            .and_then(|len| sealed.get(..len))
        else {
            continue;
        };
        if let Ok(Envelope::Sealed(read)) = Envelope::decode(black_box(datagram)) {
            checksum =
                checksum.wrapping_add(server_side.open(&read, &mut opened).unwrap_or(0) as u64);
        }
    }
    checksum
}

/// The box's and the server's transports, after a whole handshake.
fn open_session(cipher: Cipher) -> Option<(Transport, Transport)> {
    let (box_pair, server_pair) = (KeyPair::generate().ok()?, KeyPair::generate().ok()?);
    let mut box_keys = BoxKeys::new(box_pair.private, server_pair.public);
    box_keys.cipher = cipher;
    let mut server_keys = ServerKeys::new(
        server_pair.private,
        AuthorizedKeys::new(vec![box_pair.public]),
    );
    server_keys.cipher = cipher;
    let mut initiator = Initiator::start(&box_keys, 0x5EED, 1).ok()?;
    let hello = read_hello(&server_keys, 0x5EED, initiator.hello_message()).ok()?;
    let mut welcome = [0u8; WELCOME_MESSAGE_LEN];
    let server_side = hello.welcome(&server_keys, &mut welcome).ok()?;
    let box_side = initiator.read_welcome(&welcome).ok()?;
    Some((box_side, server_side))
}
