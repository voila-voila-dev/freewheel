//! One sender's session: its links and its SRT socket.

use bonding_protocol::freewheel::{LinkId, SessionId};

use super::limits::MAX_LINKS_PER_SESSION;
use super::link::Link;
use super::received_window::ReceivedWindow;
use crate::srtla::GroupHandle;

/// One sender's stream and the links carrying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub(crate) id: SessionId,
    pub(crate) handle: GroupHandle,
    pub(crate) links: Vec<Link>,
    pub(crate) srt_open: bool,
    /// The link that delivered last: SRT's answers go back through it.
    pub(crate) return_link: Option<LinkId>,
    /// Sequence of the next data packet towards the sender.
    pub(crate) next_sequence: u32,
    /// Since when it has had no link (or when it was created).
    pub(crate) empty_since_us: u64,
    /// Which recent sequences arrived, on any link.
    pub(crate) received: ReceivedWindow,
}

impl Session {
    pub(crate) fn new(id: SessionId, handle: GroupHandle, now_us: u64) -> Self {
        Self {
            id,
            handle,
            links: Vec::with_capacity(MAX_LINKS_PER_SESSION),
            srt_open: false,
            return_link: None,
            next_sequence: 0,
            empty_since_us: now_us,
            received: ReceivedWindow::new(),
        }
    }

    pub(crate) fn link_mut(&mut self, id: LinkId) -> Option<&mut Link> {
        self.links.iter_mut().find(|link| link.id == id)
    }
}
