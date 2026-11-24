//! Shared helpers: keys, a handshake run to the end, packets to seal.

#![allow(dead_code)]

use bonding_protocol::freewheel::{
    Data, Envelope, Header, MAX_DATAGRAM_LEN, Packet, Sealed, TrafficClass, WELCOME_MESSAGE_LEN,
};
use freewheel_noise::{
    AuthorizedKeys, BoxKeys, Cipher, Initiator, KeyPair, ServerKeys, Transport, read_hello,
};

/// The session every test opens.
pub const SESSION: u64 = 0x5E55_1011;

/// A box and a server that trust each other.
pub struct Peers {
    pub box_keys: BoxKeys,
    pub server_keys: ServerKeys,
    pub box_pair: KeyPair,
}

/// A box and a server with fresh keys, the box authorised, both using `cipher`.
pub fn pair(cipher: Cipher) -> Peers {
    let box_pair = KeyPair::generate().unwrap();
    let server_pair = KeyPair::generate().unwrap();
    let mut box_keys = BoxKeys::new(box_pair.private.clone(), server_pair.public);
    box_keys.cipher = cipher;
    let mut server_keys = ServerKeys::new(
        server_pair.private,
        AuthorizedKeys::new(vec![box_pair.public]),
    );
    server_keys.cipher = cipher;
    Peers {
        box_keys,
        server_keys,
        box_pair,
    }
}

/// Runs a whole handshake: the box's transport, then the server's.
pub fn open(pair: &Peers) -> (Transport, Transport) {
    let mut initiator = Initiator::start(&pair.box_keys, SESSION, 1).unwrap();
    let hello = read_hello(&pair.server_keys, SESSION, initiator.hello_message()).unwrap();
    let mut welcome = [0u8; WELCOME_MESSAGE_LEN];
    let server = hello.welcome(&pair.server_keys, &mut welcome).unwrap();
    let client = initiator.read_welcome(&welcome).unwrap();
    (client, server)
}

/// An encoded v0 data packet carrying `payload`.
pub fn data(payload: &[u8]) -> Vec<u8> {
    let packet = Packet::Data(Data {
        header: Header {
            session_id: SESSION,
            link_id: 3,
        },
        class: TrafficClass::Live,
        sequence: 77,
        sent_at_us: 5,
        payload,
    });
    let mut out = vec![0u8; packet.encoded_len()];
    packet.encode(&mut out).unwrap();
    out
}

/// `plain` sealed by `transport`.
pub fn seal(transport: &mut Transport, plain: &[u8]) -> Vec<u8> {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let len = transport.seal(plain, &mut out).unwrap();
    out[..len].to_vec()
}

/// What `transport` opens of `datagram`.
pub fn open_datagram(
    transport: &mut Transport,
    datagram: &[u8],
) -> Result<Vec<u8>, freewheel_noise::OpenError> {
    let Ok(Envelope::Sealed(sealed)) = Envelope::decode(datagram) else {
        return Err(freewheel_noise::OpenError::Forged);
    };
    open_sealed(transport, &sealed)
}

/// What `transport` opens of `sealed`.
pub fn open_sealed(
    transport: &mut Transport,
    sealed: &Sealed<'_>,
) -> Result<Vec<u8>, freewheel_noise::OpenError> {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let len = transport.open(sealed, &mut out)?;
    Ok(out[..len].to_vec())
}
