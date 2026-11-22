//! freewheel transport, sending side (M1b): per-link estimation, scheduling by expected
//! arrival, deadline-aware retransmission (`docs/technical/protocol/freewheel.md`, "First candidate").
//!
//! [`Sender`] decides; it reuses the SRTLA mode's [`SenderIo`] port (sockets, logs).
//! Nothing allocates per packet.

mod in_flight;
mod limits;
mod link;
mod link_estimate;
mod sender;
mod sent_payloads;

pub use in_flight::InFlight;
pub use limits::{
    HOUSEKEEPING_PERIOD_US, IN_FLIGHT_CAPACITY, INITIAL_RTT_US, JOIN_RETRY_US, KEEPALIVE_IDLE_US,
    LOST_CAPACITY, MIN_CONGESTION_WINDOW_BYTES, MIN_RATE_BPS, OPEN_RETRY_US, RATE_SAMPLE_PERIOD_US,
    RATE_SAMPLES, RETRANSMIT_DEADLINE_US, SENT_PAYLOADS_CAPACITY, STALL_MIN_US, STALL_RTT_MULTIPLE,
};
pub use link_estimate::LinkEstimate;
pub use sender::{FreewheelLinkStatus, Sender};
pub use sent_payloads::SentPayloads;

pub use crate::srtla::{IoFailure, LinkHandle, SenderEvent, SenderIo};
