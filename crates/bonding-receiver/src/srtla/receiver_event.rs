//! What the receiver tells its logs.

use std::net::SocketAddrV4;

use super::group_handle::GroupHandle;

/// Why a registration was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// REG1 while [`MAX_GROUPS`](super::MAX_GROUPS) groups exist.
    TooManyGroups,
    /// The address already belongs to another group (as a link, or as the one that created it).
    AddressInUse,
    /// REG2 with an id no group has: answered with `REG_NGP`.
    UnknownGroup,
    /// REG2 for a group that has [`MAX_LINKS_PER_GROUP`](super::MAX_LINKS_PER_GROUP) links.
    GroupFull,
    /// No randomness for the receiver's half of the group id.
    NoRandomness,
    /// The answer (REG2 or REG3) could not be sent.
    AnswerNotSent,
}

/// Why a group was removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupRemoval {
    /// All its links timed out, or none ever registered.
    NoLinks,
    /// Its SRT socket failed: could not open, send or read (e.g. the listener restarted).
    SrtFailed,
}

/// One line of the receiver's log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverEvent {
    /// A REG1 created a group.
    GroupRegistered {
        /// The new group.
        group: GroupHandle,
        /// Who sent the REG1.
        from: SocketAddrV4,
    },
    /// A REG1 was refused with `REG_ERR`.
    GroupRefused {
        /// Who sent the REG1.
        from: SocketAddrV4,
        /// Why.
        reason: Refusal,
    },
    /// A REG2 was answered with REG3.
    LinkRegistered {
        /// Its group.
        group: GroupHandle,
        /// The link.
        link: SocketAddrV4,
        /// `true` when the link was already in the group (a harmless re-registration).
        again: bool,
    },
    /// A REG2 was refused (`REG_NGP` for an unknown group, `REG_ERR` otherwise).
    LinkRefused {
        /// The link.
        link: SocketAddrV4,
        /// Why.
        reason: Refusal,
    },
    /// A link sent nothing for [`LINK_TIMEOUT_S`](super::LINK_TIMEOUT_S) and was forgotten.
    LinkTimedOut {
        /// Its group.
        group: GroupHandle,
        /// The link.
        link: SocketAddrV4,
    },
    /// A group was removed.
    GroupRemoved {
        /// The group.
        group: GroupHandle,
        /// Why.
        reason: GroupRemoval,
    },
    /// A datagram to a link could not be sent.
    SendToLinkFailed {
        /// The link.
        link: SocketAddrV4,
    },
}
