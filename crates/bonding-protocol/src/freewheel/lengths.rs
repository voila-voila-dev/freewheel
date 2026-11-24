//! Fixed lengths and markers of freewheel packets.

/// First byte of every freewheel v0 packet: `0xF` for freewheel, `0` for the version.
///
/// It keeps our packets apart from SRT and SRTLA on the same port: SRT data starts with a
/// top bit of 0, SRT control types are `0x80xx`–`0x8Fxx` in practice and SRTLA's `0x90xx`–`0x92xx`.
pub const MAGIC: u8 = 0xF0;
/// Length of the header every packet starts with: magic, kind, link, class, session.
pub const HEADER_LEN: usize = 12;
/// Length of a data packet before its payload: header, sequence, send time.
pub const DATA_HEADER_LEN: usize = HEADER_LEN + 8;
/// Length of an ACK before its ranges: header, then four 32-bit timing fields.
pub const ACK_HEADER_LEN: usize = HEADER_LEN + 16;
/// Length of one acknowledged range: first and last sequence, 32 bits each.
pub const SEQUENCE_RANGE_LEN: usize = 8;
/// Length of a keepalive: header and send time.
pub const KEEPALIVE_LEN: usize = HEADER_LEN + 4;
/// Length of a join request: the header alone.
pub const JOIN_LEN: usize = HEADER_LEN;
/// Length of a join acceptance: the header alone.
pub const ACCEPT_LEN: usize = HEADER_LEN;
/// Length of a close: header and reason.
pub const CLOSE_LEN: usize = HEADER_LEN + 1;
/// Largest datagram either side reads: one Ethernet MTU, as in SRTLA mode.
pub const MAX_DATAGRAM_LEN: usize = 1_500;
/// Largest payload one data packet carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_DATA_PAYLOAD_LEN: usize = MAX_DATAGRAM_LEN - DATA_HEADER_LEN;
/// Most ranges one ACK carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_ACK_RANGES: usize = (MAX_DATAGRAM_LEN - ACK_HEADER_LEN) / SEQUENCE_RANGE_LEN;
/// Length of a background payload's header before a recording chunk's bytes: its kind (one
/// byte), then the chunk's offset in the recording (64 bits).
pub const RECORDING_CHUNK_HEADER_LEN: usize = 1 + 8;
/// Most recording bytes one data packet carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_RECORDING_CHUNK_LEN: usize = MAX_DATA_PAYLOAD_LEN - RECORDING_CHUNK_HEADER_LEN;

/// First byte of every freewheel v1 packet (sealed: authenticated and encrypted, M1b step 7):
/// `0xF` for freewheel, `1` for the version. A v0 packet starts with [`MAGIC`].
pub const SEALED_MAGIC: u8 = 0xF1;
/// Kind byte of a v1 hello: the box opens a session, carrying the first message of a Noise
/// `IK` handshake. Not a v0 kind.
pub const HELLO_KIND: u8 = 0x07;
/// Kind byte of a v1 welcome: the server accepts the session, carrying the handshake's second
/// message. Not a v0 kind.
pub const WELCOME_KIND: u8 = 0x08;
/// Length of a sealed packet's nonce, in clear right after the header: 64 bits, one counter
/// per session and direction, shared by every link.
pub const NONCE_LEN: usize = 8;
/// Length of the authentication tag that ends every sealed packet (ChaCha20-Poly1305 and
/// AES-256-GCM alike).
pub const TAG_LEN: usize = 16;
/// Length of a sealed packet before its encrypted bytes: the header and the nonce, both in
/// clear and authenticated (the associated data).
pub const SEALED_HEADER_LEN: usize = HEADER_LEN + NONCE_LEN;
/// What sealing adds to a v0 packet: the nonce and the tag.
pub const SEALED_OVERHEAD_LEN: usize = NONCE_LEN + TAG_LEN;
/// Largest payload one sealed data packet carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_SEALED_DATA_PAYLOAD_LEN: usize = MAX_DATA_PAYLOAD_LEN - SEALED_OVERHEAD_LEN;
/// Most recording bytes one sealed data packet carries within [`MAX_DATAGRAM_LEN`].
pub const MAX_SEALED_RECORDING_CHUNK_LEN: usize =
    MAX_SEALED_DATA_PAYLOAD_LEN - RECORDING_CHUNK_HEADER_LEN;
/// Length of a Noise public key (X25519), in bytes.
pub const PUBLIC_KEY_LEN: usize = 32;
/// Length of what a hello's handshake message carries once decrypted: the handshake's number
/// in the session (64 bits), so a replayed hello is told from a new one.
pub const HELLO_PAYLOAD_LEN: usize = 8;
/// Length of a hello's handshake message (`IK`'s first: `e, es, s, ss`): the box's ephemeral
/// key, its static key encrypted (key and tag), the payload encrypted (payload and tag).
pub const HELLO_MESSAGE_LEN: usize =
    PUBLIC_KEY_LEN + PUBLIC_KEY_LEN + TAG_LEN + HELLO_PAYLOAD_LEN + TAG_LEN;
/// Length of a welcome's handshake message (`IK`'s second: `e, ee, se`): the server's
/// ephemeral key, then the tag of an empty payload.
pub const WELCOME_MESSAGE_LEN: usize = PUBLIC_KEY_LEN + TAG_LEN;
/// Length of a hello: the header and its handshake message.
pub const HELLO_LEN: usize = HEADER_LEN + HELLO_MESSAGE_LEN;
/// Length of a welcome: the header and its handshake message.
pub const WELCOME_LEN: usize = HEADER_LEN + WELCOME_MESSAGE_LEN;
