//! An open session's keys: every packet sealed and opened with them.

use bonding_protocol::freewheel::{HEADER_LEN, SEALED_HEADER_LEN, Sealed, TAG_LEN, frame_sealed};

use super::aead::{Aead, KEY_LEN};
use super::cipher::Cipher;
use super::open_error::OpenError;
use super::replay_window::ReplayWindow;
use super::seal_error::SealError;

/// One session's transport, on one side: the key it seals with and the nonce counter (shared by
/// every link), the key it opens with and the replay window. Made by a handshake
/// ([`Initiator::read_welcome`](super::Initiator::read_welcome),
/// [`Hello::welcome`](super::Hello::welcome)); nothing allocates per packet.
pub struct Transport {
    sending: Aead,
    receiving: Aead,
    next_nonce: u64,
    window: ReplayWindow,
}

impl core::fmt::Debug for Transport {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Transport")
            .field("next_nonce", &self.next_nonce)
            .finish_non_exhaustive()
    }
}

impl Transport {
    /// A transport from a handshake's two keys, with a replay window of `window_packets`.
    pub(crate) fn new(
        cipher: Cipher,
        sending_key: &[u8; KEY_LEN],
        receiving_key: &[u8; KEY_LEN],
        window_packets: usize,
    ) -> Self {
        Self {
            sending: Aead::new(cipher, sending_key),
            receiving: Aead::new(cipher, receiving_key),
            next_nonce: 0,
            window: ReplayWindow::new(window_packets),
        }
    }

    /// Seals `plain` (an encoded v0 packet) into `out` with the next nonce: its header in
    /// clear with the v1 magic, the nonce, the rest encrypted, the tag. Returns its length,
    /// [`SEALED_OVERHEAD_LEN`](bonding_protocol::freewheel::SEALED_OVERHEAD_LEN) more than
    /// `plain`'s.
    ///
    /// # Errors
    /// `plain` cannot be framed, or the AEAD refused; the nonce is used up either way.
    pub fn seal(&mut self, plain: &[u8], out: &mut [u8]) -> Result<usize, SealError> {
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce.wrapping_add(1);
        let len = frame_sealed(plain, nonce, out).map_err(SealError::Frame)?;
        let sealed = out.get_mut(..len).ok_or(SealError::Aead)?;
        let (associated_data, rest) = sealed.split_at_mut(SEALED_HEADER_LEN);
        let (body, tag) = rest.split_at_mut(rest.len() - TAG_LEN);
        let sealed_tag = self
            .sending
            .seal(nonce, associated_data, body)
            .ok_or(SealError::Aead)?;
        tag.copy_from_slice(&sealed_tag);
        Ok(len)
    }

    /// Opens `sealed` into `out`: checks its nonce against the replay window, its tag over the
    /// clear header, the nonce and the rest, then writes the v0 packet inside (to read with
    /// [`Packet::decode`](bonding_protocol::freewheel::Packet::decode)) and returns its length.
    /// A failure costs one tag check at most, and changes nothing.
    ///
    /// # Errors
    /// See [`OpenError`].
    pub fn open(&mut self, sealed: &Sealed<'_>, out: &mut [u8]) -> Result<usize, OpenError> {
        if !self.window.is_fresh(sealed.nonce) {
            return Err(OpenError::Replayed);
        }
        let len = sealed.plain_len();
        let out = out.get_mut(..len).ok_or(OpenError::BufferTooSmall)?;
        let (header, body) = out.split_at_mut(HEADER_LEN);
        let (encrypted, tag) = sealed
            .ciphertext
            .split_at_checked(sealed.ciphertext.len().saturating_sub(TAG_LEN))
            .ok_or(OpenError::Forged)?;
        let tag: &[u8; TAG_LEN] = tag.try_into().map_err(|_| OpenError::Forged)?;
        body.copy_from_slice(encrypted);
        if !self
            .receiving
            .open(sealed.nonce, &sealed.associated_data(), body, tag)
        {
            return Err(OpenError::Forged);
        }
        header.copy_from_slice(&sealed.plain_header());
        self.window.mark(sealed.nonce);
        Ok(len)
    }

    /// The nonce the next sealed packet takes.
    #[must_use]
    pub const fn next_nonce(&self) -> u64 {
        self.next_nonce
    }
}
