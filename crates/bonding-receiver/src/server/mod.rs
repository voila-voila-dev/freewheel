//! Runs the receivers ([`srtla::Receiver`](crate::srtla::Receiver),
//! [`freewheel::Receiver`](crate::freewheel::Receiver)) on real UDP sockets, with one thread.

mod arguments;
mod event_loop;
mod freewheel_event_loop;
mod log_line;
mod udp_io;

pub use arguments::{ArgumentError, ServerConfig};
pub use event_loop::{WAKE_INTERVAL, serve};
pub use freewheel_event_loop::{FREEWHEEL_WAKE_INTERVAL, serve_freewheel};
pub use log_line::log_line;
pub use udp_io::UdpIo;
