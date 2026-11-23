//! freewheel transport, sending side (M1b, first candidate): per-link estimation and
//! scheduling by expected arrival (`docs/technical/protocol/freewheel.md`, "First candidate"),
//! capacity reports for the encoder and capacity probing ("Capacity probing").
//!
//! [`Sender`] decides; it reuses the SRTLA mode's [`SenderIo`] port (sockets, logs).
//! Nothing allocates per packet.

mod in_flight;
mod limits;
mod link;
mod link_estimate;
mod probe;
mod sender;

pub use in_flight::InFlight;
pub use limits::{
    HOUSEKEEPING_PERIOD_US, IN_FLIGHT_CAPACITY, INITIAL_RTT_US, JOIN_RETRY_US, KEEPALIVE_IDLE_US,
    MIN_CONGESTION_WINDOW_BYTES, MIN_RATE_BPS, OPEN_RETRY_US, RATE_SAMPLE_PERIOD_US, RATE_SAMPLES,
    STALL_MIN_US, STALL_RTT_MULTIPLE,
};
pub use link_estimate::LinkEstimate;
pub use probe::{
    AT_MAXIMUM_PERCENT, PROBE_BUDGET_DEPTH_US, PROBE_BUDGET_PERCENT, PROBE_DURATION_US, PROBE_GAIN,
    PROBE_INTERVAL_US, PROBE_MIN_SAMPLE_US, PROBE_SETTLE_US, PROBED_RATE_MEMORY_US, ProbeTarget,
};
pub use sender::{FreewheelLinkStatus, QUEUING_ARRIVAL_US, Sender};

pub use crate::srtla::{IoFailure, LinkHandle, SenderEvent, SenderIo};
