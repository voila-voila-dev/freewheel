//! Deterministic emulation of cellular links, for tests and benchmarks without hardware.
//!
//! - [`Link`]: one direction of a cellular link: capacity replayed from a [`Trace`]
//!   (Mahimahi format), finite modem buffer (bufferbloat), propagation delay, bursty loss
//!   ([`GilbertElliott`]), and [`Event`]s: outages, handovers, address changes, modem reboots.
//! - [`Path`]: one SIM: uplink + downlink sharing the same events, with a radio power
//!   model ([`RadioPower`]) and a data meter ([`DataMeter`]).
//! - [`Tower`]: links attached to the same cell degrade together.
//! - [`RateModel`]: synthetic traces, fitted on public traces or written by hand.
//! - [`scenarios`]: ready-made days on the road.
//!
//! Same inputs and seed give the same result on every machine.
//! Guide: `docs/technical/link-emulator.md`.

mod data_meter;
mod departure;
mod event;
mod fate;
mod gilbert_elliott;
mod link;
mod link_config;
mod path;
mod radio_power;
mod random;
mod rate_model;
pub mod scenarios;
mod send_result;
mod tower;
mod trace;
mod trace_error;

pub use data_meter::DataMeter;
pub use departure::Departure;
pub use event::Event;
pub use fate::Fate;
pub use gilbert_elliott::GilbertElliott;
pub use link::Link;
pub use link_config::LinkConfig;
pub use path::Path;
pub use radio_power::RadioPower;
pub use rate_model::RateModel;
pub use send_result::SendResult;
pub use tower::Tower;
pub use trace::{OPPORTUNITY_BYTES, Trace};
pub use trace_error::TraceError;
