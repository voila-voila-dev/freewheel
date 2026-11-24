//! What an encrypted session keeps of its handshake: who opened it, and its keys.

use bonding_protocol::freewheel::{HELLO_MESSAGE_LEN, Sealed, WELCOME_MESSAGE_LEN};
use freewheel_noise::{OpenError, PublicKey, Transport};

/// An encrypted session's handshake and keys.
#[derive(Debug)]
pub(crate) struct SessionSecurity {
    /// The box that opened it: a hello from any other key for this session id is refused.
    pub(crate) box_key: PublicKey,
    /// The number of the handshake that set the keys: a hello with a number not above it is a
    /// replay.
    pub(crate) handshake_number: u64,
    /// That handshake's hello as it came: a copy (another link, a retry after a lost welcome)
    /// is answered with the same welcome, without a new handshake.
    pub(crate) hello: [u8; HELLO_MESSAGE_LEN],
    /// The welcome that answered it.
    pub(crate) welcome: [u8; WELCOME_MESSAGE_LEN],
    /// The keys.
    pub(crate) transport: Transport,
    /// The keys before the last handshake, until a packet opens with the new ones: what the
    /// box sealed before it read the new welcome still opens.
    pub(crate) previous: Option<Transport>,
}

impl SessionSecurity {
    /// Opens `sealed` with the session's keys, or the previous ones while they last.
    pub(crate) fn open(&mut self, sealed: &Sealed<'_>, out: &mut [u8]) -> Result<usize, OpenError> {
        match self.transport.open(sealed, out) {
            Ok(len) => {
                self.previous = None;
                Ok(len)
            }
            Err(error) => self
                .previous
                .as_mut()
                .map_or(Err(error), |previous| previous.open(sealed, out)),
        }
    }
}
