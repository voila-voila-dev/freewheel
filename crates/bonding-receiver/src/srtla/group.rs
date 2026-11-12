//! A connection group: the links of one sender, and its socket to the SRT listener.

use std::net::SocketAddrV4;

use bonding_protocol::srtla::GroupId;

use super::group_handle::GroupHandle;
use super::limits::MAX_LINKS_PER_GROUP;
use super::link::Link;

/// One sender's links, gathered into one SRT stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group {
    pub(crate) handle: GroupHandle,
    pub(crate) id: GroupId,
    pub(crate) created_s: u64,
    /// Oldest first; upstream looks them up newest first.
    pub(crate) links: Vec<Link>,
    /// Where SRT packets other than ACKs go back: the link that last carried an SRT packet,
    /// the last one registered, or, before any, the address that sent REG1.
    pub(crate) last_address: SocketAddrV4,
    pub(crate) srt_open: bool,
}

impl Group {
    pub(crate) fn new(
        handle: GroupHandle,
        id: GroupId,
        created_s: u64,
        from: SocketAddrV4,
    ) -> Self {
        Self {
            handle,
            id,
            created_s,
            links: Vec::with_capacity(MAX_LINKS_PER_GROUP),
            last_address: from,
            srt_open: false,
        }
    }

    /// Compares ids in constant time: the id is the group's only secret.
    pub(crate) fn has_id(&self, id: &GroupId) -> bool {
        self.id
            .iter()
            .zip(id)
            .fold(0u8, |difference, (mine, theirs)| {
                difference | (mine ^ theirs)
            })
            == 0
    }
}
