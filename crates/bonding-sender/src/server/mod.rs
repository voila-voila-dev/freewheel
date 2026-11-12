//! Runs the [`Sender`](crate::srtla::Sender) on real UDP sockets, with one thread.

mod arguments;
mod event_loop;
mod log_line;
mod serve_error;
mod source_list;
mod udp_io;

pub use arguments::{ArgumentError, SenderConfig};
pub use event_loop::{WAKE_INTERVAL, serve};
pub use log_line::log_line;
pub use serve_error::ServeError;
pub use source_list::{parse_source_list, read_source_list};
pub use udp_io::{SEND_BUFFER_BYTES, UdpIo};
