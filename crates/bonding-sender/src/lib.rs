//! Bonding sender: runs on the box, takes the stream of one SRT caller and spreads it over every
//! modem, each with its own UDP socket.
//!
//! [`srtla::Sender`] is the SRTLA compatibility mode as a pure state machine (no sockets, no
//! clock); [`server`] runs it on UDP sockets with one thread.

pub mod server;
pub mod srtla;
