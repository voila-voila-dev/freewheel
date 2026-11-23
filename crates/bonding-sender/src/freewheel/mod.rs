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
mod recording_settings;
mod sender;
mod setting_error;

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
pub use recording_settings::{
    CHUNK_BYTES_VARIABLE, DEFAULT_CHUNK_BYTES, DEFAULT_DRAIN_US, DEFAULT_END_IDLE_US,
    DEFAULT_IN_FLIGHT_CHUNKS, DEFAULT_LOSS_TIMEOUT_US, DEFAULT_SHARE_PERCENT, DEFAULT_WINDOW_BYTES,
    DRAIN_S_VARIABLE, END_IDLE_MS_VARIABLE, IN_FLIGHT_CHUNKS_VARIABLE, LOSS_TIMEOUT_MS_VARIABLE,
    RECORDING_VARIABLE, RecordingSettings, SHARE_PERCENT_VARIABLE, WINDOW_BYTES_VARIABLE,
};
pub use sender::{FreewheelLinkStatus, QUEUING_ARRIVAL_US, Sender};
pub use setting_error::SettingError;

pub use crate::srtla::{IoFailure, LinkHandle, SenderEvent, SenderIo};
