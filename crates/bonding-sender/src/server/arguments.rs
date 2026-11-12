//! The command line, the same as upstream's:
//! `bonding-sender SRT_LISTEN_PORT SRTLA_HOST SRTLA_PORT BIND_IPS_FILE`.

use std::fmt;
use std::net::{SocketAddr, SocketAddrV4, ToSocketAddrs};
use std::path::PathBuf;

/// What the sender needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderConfig {
    /// UDP port the SRT caller sends to.
    pub listen_port: u16,
    /// Every IPv4 address the receiver's host resolves to, tried in order.
    pub receivers: Vec<SocketAddrV4>,
    /// The file listing one source address per line, reread on `SIGHUP`.
    pub source_list: PathBuf,
}

/// A command line that cannot start the sender.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentError {
    /// Not exactly four arguments.
    Usage,
    /// A port that is not a number from 1 to 65535.
    BadPort(String),
    /// The receiver's host has no IPv4 address.
    UnresolvedHost(String),
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(
                formatter,
                "usage: bonding-sender SRT_LISTEN_PORT SRTLA_HOST SRTLA_PORT BIND_IPS_FILE"
            ),
            Self::BadPort(port) => write!(formatter, "not a port: {port}"),
            Self::UnresolvedHost(host) => write!(formatter, "no IPv4 address for {host}"),
        }
    }
}

impl std::error::Error for ArgumentError {}

impl SenderConfig {
    /// Reads the arguments after the program name and resolves the receiver.
    ///
    /// # Errors
    /// See [`ArgumentError`].
    pub fn from_arguments(arguments: &[String]) -> Result<Self, ArgumentError> {
        let [listen_port, receiver_host, receiver_port, source_list] = arguments else {
            return Err(ArgumentError::Usage);
        };
        let listen_port = parse_port(listen_port)?;
        let receiver_port = parse_port(receiver_port)?;
        let receivers: Vec<SocketAddrV4> = (receiver_host.as_str(), receiver_port)
            .to_socket_addrs()
            .map(|addresses| {
                addresses
                    .filter_map(|address| match address {
                        SocketAddr::V4(v4) => Some(v4),
                        SocketAddr::V6(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        if receivers.is_empty() {
            return Err(ArgumentError::UnresolvedHost(receiver_host.clone()));
        }
        Ok(Self {
            listen_port,
            receivers,
            source_list: PathBuf::from(source_list),
        })
    }
}

fn parse_port(text: &str) -> Result<u16, ArgumentError> {
    text.parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| ArgumentError::BadPort(text.to_owned()))
}
