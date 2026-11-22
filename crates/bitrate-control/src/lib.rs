//! Adaptive bitrate controller: the encoder's bitrate follows what the transport says the
//! links carry (M1b step 5, `docs/technical/protocol/freewheel.md`, "Cross-layer bitrate").
//!
//! Pure: it takes capacity reports and a time, returns the next bitrate. No I/O.
#![no_std]

mod bounds;
mod capacity_report;
mod controller;

pub use bounds::Bounds;
pub use capacity_report::{CapacityReport, MAX_REPORT_LEN};
pub use controller::{Controller, ControllerSettings};
