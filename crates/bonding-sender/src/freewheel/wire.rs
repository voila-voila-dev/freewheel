//! How the sender's packets leave and arrive: v0 in clear, or v1 sealed with the session's keys.

use bonding_protocol::freewheel::{
    Envelope, HELLO_MESSAGE_LEN, MAX_DATA_PAYLOAD_LEN, MAX_RECORDING_CHUNK_LEN,
    MAX_SEALED_DATA_PAYLOAD_LEN, MAX_SEALED_RECORDING_CHUNK_LEN, SEALED_OVERHEAD_LEN, SessionId,
    WELCOME_MESSAGE_LEN,
};
use freewheel_noise::{BoxKeys, Initiator, OpenError, Transport};

use super::limits::REHANDSHAKE_SILENCE_US;

/// The sender's wire: in clear (v0), or encrypted (v1, M1b step 7).
#[derive(Debug)]
pub(crate) enum Wire {
    /// v0, in clear (`FREEWHEEL_PLAINTEXT=1`).
    Plain,
    /// v1: a Noise handshake opens the session, every packet after it is sealed.
    Sealed(Box<BoxSecurity>),
}

/// What a datagram from the receiver came to, through the wire.
pub(crate) enum Incoming {
    /// A v0 packet of this length: in the datagram itself (in clear), or opened into the
    /// buffer given.
    Packet(PacketBytes),
    /// A welcome on the link of this id: the session is open (again), with new keys.
    Welcome(u8),
    /// Nothing to read: not for us, a replay, or not authentic.
    Dropped,
}

/// Where an incoming v0 packet is.
pub(crate) enum PacketBytes {
    /// The datagram itself.
    Datagram,
    /// The first this many bytes of the buffer given.
    Opened(usize),
}

impl Wire {
    /// Largest live or reliable payload a data packet carries.
    pub(crate) const fn max_data_payload_len(&self) -> usize {
        match self {
            Self::Plain => MAX_DATA_PAYLOAD_LEN,
            Self::Sealed(_) => MAX_SEALED_DATA_PAYLOAD_LEN,
        }
    }

    /// Most recording bytes a data packet carries.
    pub(crate) const fn max_recording_chunk_len(&self) -> usize {
        match self {
            Self::Plain => MAX_RECORDING_CHUNK_LEN,
            Self::Sealed(_) => MAX_SEALED_RECORDING_CHUNK_LEN,
        }
    }

    /// What the wire adds to an encoded v0 packet, in bytes.
    pub(crate) const fn overhead_bytes(&self) -> usize {
        match self {
            Self::Plain => 0,
            Self::Sealed(_) => SEALED_OVERHEAD_LEN,
        }
    }

    /// The datagram that carries the encoded v0 packet `plain`: itself in clear, sealed into
    /// `out` when encrypted; `None` when encrypted and no session is open yet.
    pub(crate) fn outgoing<'a>(&mut self, plain: &'a [u8], out: &'a mut [u8]) -> Option<&'a [u8]> {
        match self {
            Self::Plain => Some(plain),
            Self::Sealed(security) => {
                let transport = security.transport.as_mut()?;
                let len = transport.seal(plain, out).ok()?;
                out.get(..len)
            }
        }
    }

    /// Reads a datagram of session `session_id` from the receiver: a v0 packet (opened into
    /// `out` when encrypted), a welcome, or nothing.
    pub(crate) fn incoming(
        &mut self,
        now_us: u64,
        session_id: SessionId,
        datagram: &[u8],
        out: &mut [u8],
    ) -> Incoming {
        let Self::Sealed(security) = self else {
            return Incoming::Packet(PacketBytes::Datagram);
        };
        match Envelope::decode(datagram) {
            Ok(envelope) if envelope.header().session_id != session_id => Incoming::Dropped,
            Ok(Envelope::Welcome(welcome)) => {
                // A copy of the welcome that opened the session (the hello went on every link,
                // each is answered): that link is accepted too.
                if security.welcome.as_slice() == welcome.message {
                    return Incoming::Welcome(welcome.header.link_id);
                }
                let Some(initiator) = security.initiator.as_mut() else {
                    return Incoming::Dropped;
                };
                let Ok(transport) = initiator.read_welcome(welcome.message) else {
                    security.rejected_packets += 1;
                    return Incoming::Dropped;
                };
                security.welcome.copy_from_slice(welcome.message);
                security.previous = security.transport.replace(transport);
                security.initiator = None;
                security.last_opened_us = now_us;
                Incoming::Welcome(welcome.header.link_id)
            }
            Ok(Envelope::Sealed(sealed)) => {
                let opened = security
                    .transport
                    .as_mut()
                    .map_or(Err(OpenError::Forged), |transport| {
                        transport.open(&sealed, out)
                    });
                let opened = match (opened, security.previous.as_mut()) {
                    (Ok(len), _) => {
                        security.previous = None;
                        Ok(len)
                    }
                    (Err(_), Some(previous)) => previous.open(&sealed, out),
                    (Err(error), None) => Err(error),
                };
                if let Ok(len) = opened {
                    security.last_opened_us = now_us;
                    Incoming::Packet(PacketBytes::Opened(len))
                } else {
                    security.rejected_packets += 1;
                    Incoming::Dropped
                }
            }
            Ok(Envelope::Hello(_)) | Err(_) => Incoming::Dropped,
        }
    }
}

/// The box's side of an encrypted session: its keys, the handshake under way, the session's
/// transport.
pub(crate) struct BoxSecurity {
    keys: BoxKeys,
    session_id: SessionId,
    /// The handshake waiting for a welcome.
    initiator: Option<Initiator>,
    /// The number of the last handshake started.
    handshake_number: u64,
    /// The keys of the open session.
    transport: Option<Transport>,
    /// The welcome that opened it (zeros before): its copies on other links accept them.
    welcome: [u8; WELCOME_MESSAGE_LEN],
    /// The keys before the last handshake, until a packet opens with the new ones.
    previous: Option<Transport>,
    /// When a packet from the receiver last opened (or a welcome came).
    last_opened_us: u64,
    /// Datagrams from the receiver dropped: replays, forgeries, welcomes that did not
    /// authenticate.
    pub(crate) rejected_packets: u64,
}

impl core::fmt::Debug for BoxSecurity {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BoxSecurity")
            .field("session_id", &self.session_id)
            .field("handshake_number", &self.handshake_number)
            .field("open", &self.transport.is_some())
            .finish_non_exhaustive()
    }
}

impl BoxSecurity {
    /// No session yet: the first housekeeping starts the handshake.
    pub(crate) const fn new(keys: BoxKeys, session_id: SessionId) -> Self {
        Self {
            keys,
            session_id,
            initiator: None,
            handshake_number: 0,
            transport: None,
            welcome: [0; WELCOME_MESSAGE_LEN],
            previous: None,
            last_opened_us: 0,
            rejected_packets: 0,
        }
    }

    /// The hello to send on every open link while a handshake waits for its welcome. A
    /// handshake starts when there is no session yet, and again when nothing from the receiver
    /// has opened for [`REHANDSHAKE_SILENCE_US`] (it may have lost the session: a restart);
    /// the session's keys stay in use until a welcome brings new ones. `None`: no handshake
    /// under way (or no randomness to start one; tried again at the next call).
    pub(crate) fn hello(&mut self, now_us: u64) -> Option<&[u8; HELLO_MESSAGE_LEN]> {
        let silent = self.transport.is_none()
            || now_us.saturating_sub(self.last_opened_us) > REHANDSHAKE_SILENCE_US;
        if self.initiator.is_none() && silent {
            let number = self.handshake_number + 1;
            self.initiator = Initiator::start(&self.keys, self.session_id, number).ok();
            if self.initiator.is_some() {
                self.handshake_number = number;
            }
        }
        self.initiator.as_ref().map(Initiator::hello_message)
    }

    /// `true` once a handshake has opened the session.
    pub(crate) const fn is_open(&self) -> bool {
        self.transport.is_some()
    }
}
