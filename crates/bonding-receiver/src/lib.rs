//! Bonding receiver: runs on our server, gathers the links of each sender into one stream and
//! hands it to the SRT listener.
//!
//! [`srtla::Receiver`] is the SRTLA compatibility mode as a pure state machine (no sockets, no
//! clock); [`server`] runs it on UDP sockets with one thread.

pub mod server;
pub mod srtla;
