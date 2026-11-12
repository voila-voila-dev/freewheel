//! Every datagram that travels between sender and receiver, decoded and encoded.

use super::decode_error::DecodeError;
use super::encode_error::EncodeError;
use super::keepalive::Keepalive;
use super::lengths::{
    ID_LEN, MAX_DATAGRAM_LEN, REG_ANSWER_LEN, REG1_LEN, REG2_LEN, REG3_LEN, SRT_MIN_LEN,
    SRTLA_ACK_HEADER_LEN, TYPE_LEN,
};
use super::packet_type::PacketType;
use super::read::raw_type;
use super::srt_ack::SrtAck;
use super::srt_control::SrtControl;
use super::srt_data::SrtData;
use super::srt_nak::SrtNak;
use super::srtla_ack::SrtlaAck;

/// The 256-byte connection group id: sender half first, receiver half second.
pub type GroupId = [u8; ID_LEN];

const CONTROL_BIT: u8 = 0x80;

/// One datagram, borrowed from the buffer it was read from.
///
/// SRT packets keep their bytes, since they are forwarded unchanged; SRTLA packets are
/// reduced to their fields. Decoding then encoding gives back the same bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packet<'a> {
    /// SRT data packet.
    SrtData(SrtData<'a>),
    /// SRT ACK, read by the sender to steer its links.
    SrtAck(SrtAck<'a>),
    /// SRT NAK, read by the sender to steer its links.
    SrtNak(SrtNak<'a>),
    /// Any other SRT control packet.
    SrtControl(SrtControl<'a>),
    /// SRTLA keepalive.
    Keepalive(Keepalive<'a>),
    /// SRTLA ACK.
    SrtlaAck(SrtlaAck<'a>),
    /// REG1: the sender proposes a group id (only its first half is kept).
    Reg1(&'a GroupId),
    /// REG2: the full group id, from the receiver or for each link of the sender.
    Reg2(&'a GroupId),
    /// REG3: a link is registered.
    Reg3,
    /// `REG_ERR`: registration refused.
    RegErr,
    /// `REG_NGP`: no such group.
    RegNgp,
    /// `REG_NAK`: defined upstream, never sent.
    RegNak,
}

impl<'a> Packet<'a> {
    /// Reads one datagram.
    ///
    /// Same rules as upstream: registration packets must have their exact length, and a
    /// packet typed as SRTLA but of the wrong length is an SRT packet if it is long enough.
    ///
    /// # Errors
    /// See [`DecodeError`].
    pub fn decode(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        let len = bytes.len();
        if len > MAX_DATAGRAM_LEN {
            return Err(DecodeError::TooLong { len });
        }
        let raw = raw_type(bytes).ok_or(DecodeError::TooShort { len })?;
        if let Some(packet) = Self::decode_srtla(raw, bytes)? {
            return Ok(packet);
        }
        Self::decode_srt(raw, bytes).ok_or(DecodeError::TooShort { len })
    }

    fn decode_srtla(raw: u16, bytes: &'a [u8]) -> Result<Option<Self>, DecodeError> {
        let after_type = bytes.get(TYPE_LEN..).unwrap_or_default();
        let group_id = || <&GroupId>::try_from(after_type).ok();
        let Some(kind) = PacketType::from_raw(raw) else {
            return Ok(None);
        };
        Ok(match (kind, bytes.len()) {
            (PacketType::Keepalive, _) => Some(Self::Keepalive(Keepalive {
                payload: after_type,
            })),
            (PacketType::SrtlaAck, _) => Some(Self::SrtlaAck(
                SrtlaAck::parse(bytes).ok_or(DecodeError::MalformedSrtlaAck)?,
            )),
            (PacketType::Reg1, REG1_LEN) => group_id().map(Self::Reg1),
            (PacketType::Reg2, REG2_LEN) => group_id().map(Self::Reg2),
            (PacketType::Reg3, REG3_LEN) => Some(Self::Reg3),
            (PacketType::RegErr, REG_ANSWER_LEN) => Some(Self::RegErr),
            (PacketType::RegNgp, REG_ANSWER_LEN) => Some(Self::RegNgp),
            (PacketType::RegNak, REG_ANSWER_LEN) => Some(Self::RegNak),
            _ => None,
        })
    }

    fn decode_srt(raw: u16, bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < SRT_MIN_LEN {
            return None;
        }
        let is_control = bytes.first().is_some_and(|first| first & CONTROL_BIT != 0);
        if !is_control {
            return SrtData::parse(bytes).map(Self::SrtData);
        }
        let parsed = match PacketType::from_raw(raw) {
            Some(PacketType::SrtAck) => SrtAck::parse(bytes).map(Self::SrtAck),
            Some(PacketType::SrtNak) => SrtNak::parse(bytes).map(Self::SrtNak),
            _ => None,
        };
        Some(parsed.unwrap_or(Self::SrtControl(SrtControl::new(bytes))))
    }

    /// Its control type; `None` for data packets and for SRT control packets of other types
    /// (including SRTLA types of the wrong length, which upstream forwards to SRT).
    #[must_use]
    pub fn packet_type(&self) -> Option<PacketType> {
        match self {
            Self::SrtData(_) => None,
            Self::SrtAck(_) => Some(PacketType::SrtAck),
            Self::SrtNak(_) => Some(PacketType::SrtNak),
            Self::SrtControl(control) => {
                PacketType::from_raw(control.raw_type()).filter(|kind| !kind.is_srtla())
            }
            Self::Keepalive(_) => Some(PacketType::Keepalive),
            Self::SrtlaAck(_) => Some(PacketType::SrtlaAck),
            Self::Reg1(_) => Some(PacketType::Reg1),
            Self::Reg2(_) => Some(PacketType::Reg2),
            Self::Reg3 => Some(PacketType::Reg3),
            Self::RegErr => Some(PacketType::RegErr),
            Self::RegNgp => Some(PacketType::RegNgp),
            Self::RegNak => Some(PacketType::RegNak),
        }
    }

    /// The bytes to forward to the other SRT end, for SRT packets; `None` for SRTLA's own.
    #[must_use]
    pub const fn srt_bytes(&self) -> Option<&'a [u8]> {
        match self {
            Self::SrtData(data) => Some(data.bytes()),
            Self::SrtAck(ack) => Some(ack.bytes()),
            Self::SrtNak(nak) => Some(nak.bytes()),
            Self::SrtControl(control) => Some(control.bytes()),
            _ => None,
        }
    }

    /// Length of its encoding, in bytes.
    #[must_use]
    pub const fn encoded_len(&self) -> usize {
        if let Some(bytes) = self.srt_bytes() {
            return bytes.len();
        }
        match self {
            Self::Keepalive(keepalive) => TYPE_LEN + keepalive.payload.len(),
            Self::SrtlaAck(ack) => SRTLA_ACK_HEADER_LEN + ack.sequence_number_bytes().len(),
            Self::Reg1(_) => REG1_LEN,
            Self::Reg2(_) => REG2_LEN,
            _ => TYPE_LEN,
        }
    }

    /// Writes the packet at the start of `out` and returns its length.
    ///
    /// # Errors
    /// `out` too small, or a keepalive payload too long for one datagram.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let len = self.encoded_len();
        if len > MAX_DATAGRAM_LEN {
            return Err(EncodeError::TooLong { len });
        }
        let out = out
            .get_mut(..len)
            .ok_or(EncodeError::BufferTooSmall { needed: len })?;
        if let Some(bytes) = self.srt_bytes() {
            out.copy_from_slice(bytes);
            return Ok(len);
        }
        let (kind, rest) = out.split_at_mut(TYPE_LEN);
        let raw = self.packet_type().map_or(0, PacketType::raw);
        kind.copy_from_slice(&raw.to_be_bytes());
        match self {
            Self::Keepalive(keepalive) => rest.copy_from_slice(keepalive.payload),
            Self::SrtlaAck(ack) => {
                let (padding, words) = rest.split_at_mut(SRTLA_ACK_HEADER_LEN - TYPE_LEN);
                padding.fill(0);
                words.copy_from_slice(ack.sequence_number_bytes());
            }
            Self::Reg1(id) | Self::Reg2(id) => rest.copy_from_slice(id.as_slice()),
            _ => {}
        }
        Ok(len)
    }
}
