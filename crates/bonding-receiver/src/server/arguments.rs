//! The command line, the same as upstream's: `bonding-receiver LISTEN_PORT SRT_HOST SRT_PORT`.

use std::fmt;
use std::net::{SocketAddr, SocketAddrV4, ToSocketAddrs};

/// What the server needs to start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerConfig {
    /// UDP port the senders' links reach.
    pub listen_port: u16,
    /// The SRT listener every group is forwarded to.
    pub srt_address: SocketAddrV4,
}

/// A command line that cannot start the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentError {
    /// Not exactly three arguments.
    Usage,
    /// A port that is not a number from 1 to 65535.
    BadPort(String),
    /// The SRT host has no IPv4 address.
    UnresolvedHost(String),
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(
                formatter,
                "usage: bonding-receiver LISTEN_PORT SRT_HOST SRT_PORT"
            ),
            Self::BadPort(port) => write!(formatter, "not a port: {port}"),
            Self::UnresolvedHost(host) => write!(formatter, "no IPv4 address for {host}"),
        }
    }
}

impl ServerConfig {
    /// Reads the arguments after the program name.
    ///
    /// # Errors
    /// See [`ArgumentError`].
    pub fn from_arguments(arguments: &[String]) -> Result<Self, ArgumentError> {
        let [listen_port, srt_host, srt_port] = arguments else {
            return Err(ArgumentError::Usage);
        };
        let listen_port = parse_port(listen_port)?;
        let srt_port = parse_port(srt_port)?;
        let srt_address = (srt_host.as_str(), srt_port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addresses| {
                addresses.find_map(|address| match address {
                    SocketAddr::V4(v4) => Some(v4),
                    SocketAddr::V6(_) => None,
                })
            })
            .ok_or_else(|| ArgumentError::UnresolvedHost(srt_host.clone()))?;
        Ok(Self {
            listen_port,
            srt_address,
        })
    }
}

fn parse_port(text: &str) -> Result<u16, ArgumentError> {
    text.parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| ArgumentError::BadPort(text.to_owned()))
}

impl std::error::Error for ArgumentError {}
