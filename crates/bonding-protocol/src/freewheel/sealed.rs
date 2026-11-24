//! A sealed packet: a v0 packet authenticated and encrypted, its header in clear.

use super::encode_error::EncodeError;
use super::header::Header;
use super::lengths::{
    HEADER_LEN, MAGIC, MAX_DATAGRAM_LEN, NONCE_LEN, SEALED_HEADER_LEN, SEALED_MAGIC, TAG_LEN,
};
use super::packet_kind::PacketKind;
use super::traffic_class::TrafficClass;

/// A sealed packet, borrowed from the datagram it was read from: the v0 packet's header in
/// clear (with [`SEALED_MAGIC`]), the nonce in clear, then the v0 packet's bytes after its
/// header, encrypted, then the tag. The header and the nonce are the associated data: the
/// first [`SEALED_HEADER_LEN`] bytes of the datagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sealed<'a> {
    /// Session and link, in clear: the server routes on them before decrypting.
    pub header: Header,
    /// The kind of the packet inside.
    pub kind: PacketKind,
    /// Its traffic class, for data; `None` for every other kind.
    pub class: Option<TrafficClass>,
    /// The nonce it was sealed with.
    pub nonce: u64,
    /// The encrypted bytes, then the [`TAG_LEN`]-byte tag.
    pub ciphertext: &'a [u8],
}

impl Sealed<'_> {
    /// The v0 header of the packet inside: what goes in front of the decrypted bytes to read
    /// them with [`Packet::decode`](super::Packet::decode).
    #[must_use]
    pub fn plain_header(&self) -> [u8; HEADER_LEN] {
        header_bytes(MAGIC, self.kind, self.class, self.header)
    }

    /// Its associated data: the clear header (with [`SEALED_MAGIC`]) and the nonce, the first
    /// [`SEALED_HEADER_LEN`] bytes of its encoding.
    #[must_use]
    pub fn associated_data(&self) -> [u8; SEALED_HEADER_LEN] {
        let mut bytes = [0u8; SEALED_HEADER_LEN];
        let (header, nonce) = bytes.split_at_mut(HEADER_LEN);
        header.copy_from_slice(&header_bytes(
            SEALED_MAGIC,
            self.kind,
            self.class,
            self.header,
        ));
        nonce.copy_from_slice(&self.nonce.to_be_bytes());
        bytes
    }

    /// Length of its encoding, in bytes.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        SEALED_HEADER_LEN + self.ciphertext.len()
    }

    /// Length of the v0 packet inside, in bytes.
    #[must_use]
    pub const fn plain_len(&self) -> usize {
        self.encoded_len() - NONCE_LEN - TAG_LEN
    }

    /// Writes the sealed packet at the start of `out` and returns its length.
    ///
    /// # Errors
    /// `out` too small, or longer than one datagram.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let len = self.encoded_len();
        if len > MAX_DATAGRAM_LEN {
            return Err(EncodeError::TooLong { len });
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        let (fixed, ciphertext) = out.split_at_mut(SEALED_HEADER_LEN);
        let (header, nonce) = fixed.split_at_mut(HEADER_LEN);
        header.copy_from_slice(&header_bytes(
            SEALED_MAGIC,
            self.kind,
            self.class,
            self.header,
        ));
        nonce.copy_from_slice(&self.nonce.to_be_bytes());
        ciphertext.copy_from_slice(self.ciphertext);
        Ok(len)
    }
}

/// Lays out the sealed form of `plain` (an encoded v0 packet) at the start of `out`: its
/// header with [`SEALED_MAGIC`], `nonce`, its bytes after the header still in clear, and room
/// for the tag. Returns the sealed length: the caller encrypts
/// `out[SEALED_HEADER_LEN..len - TAG_LEN]` in place with `out[..SEALED_HEADER_LEN]` as the
/// associated data, and writes the tag in `out[len - TAG_LEN..len]`.
///
/// # Errors
/// `plain` is not a v0 packet (shorter than its header, or without [`MAGIC`]), `out` is too
/// small, or the sealed packet would be longer than one datagram.
pub fn frame_sealed(plain: &[u8], nonce: u64, out: &mut [u8]) -> Result<usize, EncodeError> {
    let (Some(plain_header), Some(body)) = (plain.get(..HEADER_LEN), plain.get(HEADER_LEN..))
    else {
        return Err(EncodeError::NotPlain);
    };
    if plain_header.first() != Some(&MAGIC) {
        return Err(EncodeError::NotPlain);
    }
    let len = SEALED_HEADER_LEN + body.len() + TAG_LEN;
    if len > MAX_DATAGRAM_LEN {
        return Err(EncodeError::TooLong { len });
    }
    let out = out
        .get_mut(..len)
        .ok_or(EncodeError::BufferTooSmall { needed: len })?;
    let (fixed, rest) = out.split_at_mut(SEALED_HEADER_LEN);
    let (header, nonce_bytes) = fixed.split_at_mut(HEADER_LEN);
    let (encrypted, tag) = rest.split_at_mut(body.len());
    header.copy_from_slice(plain_header);
    if let Some(magic) = header.first_mut() {
        *magic = SEALED_MAGIC;
    }
    nonce_bytes.copy_from_slice(&nonce.to_be_bytes());
    encrypted.copy_from_slice(body);
    tag.fill(0);
    Ok(len)
}

/// The 12 header bytes: `magic`, kind, link, class (0 outside data), session.
pub(crate) fn header_bytes(
    magic: u8,
    kind: PacketKind,
    class: Option<TrafficClass>,
    header: Header,
) -> [u8; HEADER_LEN] {
    let mut bytes = [0u8; HEADER_LEN];
    let (start, session) = bytes.split_at_mut(4);
    start.copy_from_slice(&[
        magic,
        kind.raw(),
        header.link_id,
        class.map_or(0, TrafficClass::raw),
    ]);
    session.copy_from_slice(&header.session_id.to_be_bytes());
    bytes
}
