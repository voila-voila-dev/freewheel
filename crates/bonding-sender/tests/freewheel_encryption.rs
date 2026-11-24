//! Encryption end to end on a simulated network (M1b step 7): the session opens with one round
//! trip whatever single control packet is lost, a box the server does not know gets nowhere,
//! and an altered, forged or replayed packet is dropped while the stream goes on.

mod freewheel_network;

use std::path::PathBuf;

use bonding_protocol::freewheel::{HELLO_KIND, SEALED_MAGIC};
use bonding_sender::freewheel::RecordingSettings;
use freewheel_network::{LinkSpec, Network};
use freewheel_noise::{AuthorizedKeys, BoxKeys, Cipher, Initiator, KeyPair, ServerKeys};

const MS: u64 = 1_000;
const S: u64 = 1_000_000;
const MBPS: u64 = 1_000_000;
const STREAM_BPS: u64 = 2 * MBPS;

fn two_links() -> Network {
    Network::new(&[
        LinkSpec::new(30 * MS, 8 * MBPS),
        LinkSpec::new(60 * MS, 8 * MBPS),
    ])
}

fn encrypted(cipher: Cipher) -> Network {
    let mut network = two_links();
    network.enable_encryption(cipher);
    network
}

/// Share of what the caller sent that reached the listener.
fn delivered_share(network: &Network) -> f64 {
    f64::from(u32::try_from(network.delivered().len()).unwrap_or(u32::MAX))
        / f64::from(network.sent().max(1))
}

#[test]
fn an_encrypted_stream_arrives_whole_with_either_cipher() {
    for cipher in [Cipher::ChaCha20Poly1305, Cipher::Aes256Gcm] {
        let mut network = encrypted(cipher);
        network.run(500 * MS, 0);
        assert!(network.sender.session_open(), "{cipher:?}: opened");
        network.run(5 * S, STREAM_BPS);
        network.run(S, 0);
        assert!(delivered_share(&network) > 0.99, "{cipher:?}");
        assert!(network.usable(0) && network.usable(1));
        assert_eq!(network.sender.rejected_packets(), 0);
        assert_eq!(network.receiver.rejected_packets().replayed, 0);
        assert_eq!(network.receiver.rejected_packets().forged, 0);
        network.srt_answer();
        network.run(200 * MS, 0);
        assert_eq!(network.to_caller(), 1, "{cipher:?}: the way back works");
    }
}

#[test]
fn every_datagram_is_v1() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    network.capture_uplink();
    network.run(2 * S, STREAM_BPS);
    assert!(!network.captured().is_empty());
    for (_, datagram) in network.captured() {
        assert_eq!(datagram[0], SEALED_MAGIC);
    }
}

#[test]
fn the_session_opens_in_one_round_trip() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    // Housekeeping at 1 ms sends the hello; the 30 ms link brings the welcome back at 61 ms.
    network.run(60 * MS, 0);
    assert!(!network.sender.session_open());
    network.run(5 * MS, 0);
    assert!(network.sender.session_open());
    assert!(
        network.usable(0),
        "the hello's link is accepted by its welcome"
    );
    network.run(60 * MS, 0);
    assert!(network.usable(1), "so is every link the hello went on");
}

#[test]
fn a_lost_hello_on_one_link_costs_nothing() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    network.set_uplink_lost(0, true);
    network.run(130 * MS, 0);
    assert!(network.sender.session_open(), "opened by the other link");
    assert!(network.usable(1));
    network.set_uplink_lost(0, false);
    network.run(400 * MS, 0);
    assert!(network.usable(0), "the first link joins with a sealed join");
}

#[test]
fn a_lost_welcome_is_answered_again_on_the_retry() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    for link in 0..2 {
        network.set_downlink_lost(link, true);
    }
    network.run(100 * MS, 0);
    assert!(!network.sender.session_open());
    for link in 0..2 {
        network.set_downlink_lost(link, false);
    }
    // The hello goes again 250 ms after the first: the same bytes, the same welcome back.
    network.run(250 * MS, 0);
    assert!(network.sender.session_open());
    assert_eq!(network.receiver.session_count(), 1);
    assert_eq!(
        network.receiver.rejected_packets().hellos,
        0,
        "a copy, not a replay"
    );
    network.run(2 * S, STREAM_BPS);
    assert!(delivered_share(&network) > 0.95);
}

#[test]
fn a_box_not_on_the_list_gets_nowhere() {
    let mut network = two_links();
    let (box_keys, mut server_keys) = Network::keys(Cipher::ChaCha20Poly1305);
    server_keys.authorized = AuthorizedKeys::new(vec![
        KeyPair::generate().map_or_else(|_| unreachable!("randomness"), |pair| pair.public),
    ]);
    network.enable_encryption_with(box_keys, server_keys);
    network.run(S, 0);
    network.run(2 * S, STREAM_BPS);
    assert!(!network.sender.session_open());
    assert!(network.delivered().is_empty());
    assert_eq!(network.receiver.session_count(), 0, "no state kept");
    assert!(network.receiver.rejected_packets().hellos > 0);
}

#[test]
fn a_box_with_the_wrong_server_key_gets_nowhere() {
    let mut network = two_links();
    let (mut box_keys, server_keys) = Network::keys(Cipher::ChaCha20Poly1305);
    box_keys.server =
        KeyPair::generate().map_or_else(|_| unreachable!("randomness"), |pair| pair.public);
    network.enable_encryption_with(box_keys, server_keys);
    network.run(3 * S, STREAM_BPS);
    assert!(!network.sender.session_open());
    assert!(network.delivered().is_empty());
    assert_eq!(network.receiver.session_count(), 0);
}

#[test]
fn both_sides_must_agree_on_encryption() {
    let mut sealed_box = two_links();
    sealed_box
        .sender
        .enable_encryption(Network::keys(Cipher::ChaCha20Poly1305).0);
    sealed_box.run(2 * S, STREAM_BPS);
    assert!(
        sealed_box.delivered().is_empty(),
        "a plain server reads no v1"
    );
    let mut plain_box = two_links();
    plain_box
        .receiver
        .enable_encryption(Network::keys(Cipher::ChaCha20Poly1305).1);
    plain_box.run(2 * S, STREAM_BPS);
    assert!(
        plain_box.delivered().is_empty(),
        "an encrypted server reads no v0"
    );
    assert_eq!(plain_box.receiver.session_count(), 0);
}

#[test]
fn another_box_cannot_take_a_live_session() {
    // Even a box the server accepts: a session belongs to the key that opened it.
    let pair = || KeyPair::generate().unwrap_or_else(|_| unreachable!("randomness"));
    let (server, owner, intruder) = (pair(), pair(), pair());
    let mut network = two_links();
    network.enable_encryption_with(
        BoxKeys::new(owner.private, server.public),
        ServerKeys::new(
            server.private,
            AuthorizedKeys::new(vec![owner.public, intruder.public]),
        ),
    );
    network.run(500 * MS, STREAM_BPS);
    let session_id = network.sender.session_id();
    let initiator = Initiator::start(
        &BoxKeys::new(intruder.private, server.public),
        session_id,
        99,
    )
    .unwrap_or_else(|_| unreachable!("keys"));
    let before = network.receiver.rejected_packets().hellos;
    network.inject_to_receiver(0, &hello_datagram(session_id, 7, initiator.hello_message()));
    assert_eq!(network.receiver.rejected_packets().hellos, before + 1);
    let delivered = network.delivered().len();
    network.run(2 * S, STREAM_BPS);
    assert!(
        network.delivered().len() > delivered + 300,
        "the stream goes on"
    );
    assert_eq!(
        network.sender.rejected_packets(),
        0,
        "with the owner's keys"
    );
}

#[test]
fn a_replayed_hello_or_packet_is_dropped() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    network.capture_uplink();
    network.run(2 * S, STREAM_BPS);
    let captured = network.captured().to_vec();
    let delivered = network.delivered().len();
    let before = network.receiver.rejected_packets();
    for (link, datagram) in &captured {
        network.inject_to_receiver(*link, datagram);
    }
    let after = network.receiver.rejected_packets();
    let hellos = captured
        .iter()
        .filter(|(_, datagram)| datagram[1] == HELLO_KIND)
        .count() as u64;
    assert!(hellos > 0);
    assert_eq!(
        after.replayed - before.replayed,
        captured.len() as u64 - hellos,
        "every sealed packet is a replay"
    );
    assert_eq!(
        network.delivered().len(),
        delivered,
        "nothing reached the listener twice"
    );
    network.run(S, STREAM_BPS);
    assert!(
        network.delivered().len() > delivered + 100,
        "the stream goes on"
    );
}

#[test]
fn an_altered_header_or_payload_is_dropped() {
    let mut network = encrypted(Cipher::Aes256Gcm);
    network.run(S, STREAM_BPS);
    network.capture_uplink();
    network.run(20 * MS, STREAM_BPS);
    let (link, datagram) = network
        .captured()
        .iter()
        .find(|(_, datagram)| datagram[1] == 0x01 && datagram.len() > 100)
        .cloned()
        .unwrap_or_else(|| unreachable!("data went"));
    let delivered = network.delivered().len();
    let before = network.receiver.rejected_packets().forged;
    // Each with a nonce never used, so the replay window lets it through to the tag check:
    // the nonce alone, then the link, the class, a payload byte, the tag.
    let mut nonce = [0u8; 8];
    nonce.copy_from_slice(&datagram[12..20]);
    let fresh = (u64::from_be_bytes(nonce) + 1_000).to_be_bytes();
    for (at, flip) in [
        (12, 0),
        (2, 0x01),
        (3, 0x02),
        (30, 0x80),
        (datagram.len() - 1, 0x01),
    ] {
        let mut altered = datagram.clone();
        altered[12..20].copy_from_slice(&fresh);
        altered[at] ^= flip;
        network.inject_to_receiver(link, &altered);
    }
    assert_eq!(network.receiver.rejected_packets().forged, before + 5);
    assert_eq!(network.delivered().len(), delivered);
}

#[test]
fn reordering_across_links_stays_within_the_window() {
    let mut network = Network::new(&[
        LinkSpec::new(10 * MS, 4 * MBPS),
        LinkSpec::new(250 * MS, 4 * MBPS),
    ]);
    network.enable_encryption(Cipher::ChaCha20Poly1305);
    network.run(5 * S, 3 * MBPS);
    network.run(S, 0);
    assert!(delivered_share(&network) > 0.99);
    assert_eq!(network.receiver.rejected_packets().replayed, 0);
    assert_eq!(network.sender.rejected_packets(), 0);
}

#[test]
fn the_recording_arrives_byte_for_byte_encrypted() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    network.enable_recording(&RecordingSettings::new(PathBuf::from("/unused")));
    let recording: Vec<u8> = (0..400_000_u32)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13).to_le_bytes()[0])
        .collect();
    network.run(500 * MS, 0);
    for second in 0..4 {
        network.record(&recording[second * 100_000..(second + 1) * 100_000]);
        network.run(S, STREAM_BPS);
    }
    network.run(10 * S, 0);
    assert_eq!(network.recording_copy(), recording);
}

#[test]
fn after_the_server_restarts_the_box_opens_the_session_again() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    network.run(2 * S, STREAM_BPS);
    network.restart_receiver();
    network.run(5 * S, STREAM_BPS);
    let delivered = network.delivered().len();
    network.run(2 * S, STREAM_BPS);
    assert!(
        network.delivered().len() > delivered + 300,
        "the stream came back with a new handshake"
    );
    assert!(network.usable(0) && network.usable(1));
}

#[test]
fn a_long_outage_keeps_the_stream_when_links_come_back() {
    let mut network = encrypted(Cipher::ChaCha20Poly1305);
    network.run(2 * S, STREAM_BPS);
    for link in 0..2 {
        network.set_up(link, false);
    }
    network.run(5 * S, STREAM_BPS);
    for link in 0..2 {
        network.set_up(link, true);
    }
    network.run(S, STREAM_BPS);
    let delivered = network.delivered().len();
    network.run(2 * S, STREAM_BPS);
    assert!(network.delivered().len() > delivered + 300);
    assert_eq!(
        network.receiver.session_count(),
        1,
        "the same session, new keys"
    );
}

fn hello_datagram(session_id: u64, link_id: u8, message: &[u8]) -> Vec<u8> {
    use bonding_protocol::freewheel::{Envelope, HandshakeMessage, Header};
    let envelope = Envelope::Hello(HandshakeMessage {
        header: Header {
            session_id,
            link_id,
        },
        message,
    });
    let mut out = vec![0u8; envelope.encoded_len()];
    let _ = envelope.encode(&mut out);
    out
}
