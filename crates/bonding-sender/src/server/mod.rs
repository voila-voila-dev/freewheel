//! Runs the senders ([`srtla::Sender`](crate::srtla::Sender),
//! [`freewheel::Sender`](crate::freewheel::Sender)) on real UDP sockets, with one thread.

mod arguments;
mod event_loop;
mod freewheel_event_loop;
mod log_line;
mod recording_file;
mod serve_error;
mod source_list;
mod udp_io;

pub use arguments::{ArgumentError, SenderConfig};
pub use event_loop::{WAKE_INTERVAL, serve};
pub use freewheel_event_loop::{
    CAPACITY_REPORT_PERIOD, CapacityOutput, FREEWHEEL_WAKE_INTERVAL, random_session_id,
    serve_freewheel,
};
pub use log_line::log_line;
pub use recording_file::RecordingFile;
pub use serve_error::ServeError;
pub use source_list::{parse_source_list, read_source_list};
pub use udp_io::{SEND_BUFFER_BYTES, UdpIo};
