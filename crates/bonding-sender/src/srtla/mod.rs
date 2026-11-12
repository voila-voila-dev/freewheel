//! SRTLA compatibility mode: behaves like upstream `srtla_send`, quirks included
//! (`docs/technical/protocol/srtla.md`, "How `bonding-sender` behaves").
//!
//! [`Sender`] decides; [`SenderIo`] is everything it needs from outside (sockets, logs).
//! Nothing allocates per packet.

mod housekeeping_outcome;
mod limits;
mod link;
mod link_activity;
mod link_handle;
mod link_status;
mod packet_log;
mod sender;
mod sender_event;
mod sender_io;

pub use housekeeping_outcome::HousekeepingOutcome;
pub use limits::{
    GIVE_UP_AFTER_MS, HOUSEKEEPING_PERIOD_MS, IDLE_BEFORE_KEEPALIVE_S, LINK_TIMEOUT_S,
    PACKET_LOG_LEN, REGISTRATION_TIMEOUT_S, WINDOW_ACK_INCREASE_MILLIPACKETS,
    WINDOW_DEFAULT_MILLIPACKETS, WINDOW_MAX_MILLIPACKETS, WINDOW_MIN_MILLIPACKETS,
    WINDOW_NAK_DECREASE_MILLIPACKETS,
};
pub use link_handle::LinkHandle;
pub use link_status::LinkStatus;
pub use sender::Sender;
pub use sender_event::SenderEvent;
pub use sender_io::{IoFailure, SenderIo};
