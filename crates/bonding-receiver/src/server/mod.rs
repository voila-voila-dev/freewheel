//! Runs the [`Receiver`](crate::srtla::Receiver) on real UDP sockets, with one thread.

mod arguments;
mod event_loop;
mod log_line;
mod udp_io;

pub use arguments::{ArgumentError, ServerConfig};
pub use event_loop::{WAKE_INTERVAL, serve};
pub use log_line::log_line;
pub use udp_io::UdpIo;
