//! SRTLA compatibility mode: behaves like upstream `srtla_rec`, quirks included
//! (`docs/technical/protocol/srtla.md`, "How `bonding-receiver` behaves").
//!
//! [`Receiver`] decides; [`ReceiverIo`] is everything it needs from outside (sockets,
//! randomness, logs). Nothing allocates per packet.

mod ack_batch;
mod address_owner;
mod group;
mod group_handle;
mod limits;
mod link;
mod receiver;
mod receiver_event;
mod receiver_io;

pub use group_handle::GroupHandle;
pub use limits::{
    ACK_BATCH_LEN, CLEANUP_PERIOD_S, EMPTY_GROUP_TIMEOUT_S, LINK_TIMEOUT_S, MAX_GROUPS,
    MAX_LINKS_PER_GROUP,
};
pub use receiver::Receiver;
pub use receiver_event::{GroupRemoval, ReceiverEvent, Refusal};
pub use receiver_io::{IoFailure, ReceiverIo};
