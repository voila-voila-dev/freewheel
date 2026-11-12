//! What the sender tells its logs.

use std::net::{Ipv4Addr, SocketAddrV4};

/// One line of the sender's log. Links are named by their source address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenderEvent {
    /// A source address joined the list.
    LinkAdded {
        /// Its address.
        source: Ipv4Addr,
    },
    /// A source address left the list; its socket is closed.
    LinkRemoved {
        /// Its address.
        source: Ipv4Addr,
    },
    /// At start, a link's socket could not be bound (retried quietly every second).
    LinkNotOpened {
        /// Its address.
        source: Ipv4Addr,
    },
    /// The receiver answered our REG1: the group id is complete, REG2 goes out on every link.
    GroupRegistered {
        /// The link that carried the REG1.
        source: Ipv4Addr,
    },
    /// A REG2 answer whose first half is not ours: ignored.
    GroupIdMismatch {
        /// The link it came on.
        source: Ipv4Addr,
    },
    /// A REG3: the receiver registered this link.
    LinkEstablished {
        /// Its address.
        source: Ipv4Addr,
    },
    /// A link heard nothing for too long (or a send failed): reset, registered again.
    LinkFailed {
        /// Its address.
        source: Ipv4Addr,
    },
    /// A datagram could not be sent on a link, which stops carrying data at once.
    SendFailed {
        /// Its address.
        source: Ipv4Addr,
    },
    /// No link can carry data (logged every second once the sender has been connected).
    NoLinkAvailable,
    /// Nothing worked on the receiver's address: trying the next one it resolved to.
    TryingReceiver {
        /// The next address.
        receiver: SocketAddrV4,
    },
    /// Every link failed for too long: the sender stops.
    GaveUp {
        /// It had been connected once (upstream: "failed to re-establish").
        had_connected: bool,
    },
}
