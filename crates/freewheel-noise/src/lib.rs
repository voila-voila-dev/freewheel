//! freewheel's cryptography (M1b step 7, `docs/technical/protocol/freewheel.md`, "Wire format
//! v1"): everything that touches a key, so the sender and the receiver only call it.
//!
//! - The handshake: Noise `IK` through `snow`. The box ([`Initiator`]) knows the server's
//!   static key, so one round trip opens the session: a hello, a welcome. The server
//!   ([`read_hello`]) refuses a box whose key is not in its [`AuthorizedKeys`].
//! - The transport ([`Transport`]): every v0 packet sealed with the handshake's keys
//!   (ChaCha20-Poly1305 or AES-256-GCM, [`Cipher`]), its header and nonce authenticated as
//!   associated data, then opened through a [`ReplayWindow`]. Nothing allocates per packet.
//! - Keys: [`KeyPair`] to make one, [`PrivateKey`], [`PublicKey`] and [`AuthorizedKeys`] to read
//!   the files the binaries are given.
//!
//! No cryptography is written here: `snow` runs the handshake, `chacha20poly1305` and `aes-gcm`
//! seal (`decisions.md`, 2026-11-24).

mod aead;
mod authorized_keys;
mod box_keys;
mod cipher;
mod handshake_error;
mod hex;
mod initiator;
mod key_error;
mod key_pair;
mod open_error;
mod private_key;
mod prologue;
mod public_key;
mod replay_window;
mod responder;
mod seal_error;
mod server_keys;
mod transport;

pub use authorized_keys::AuthorizedKeys;
pub use box_keys::BoxKeys;
pub use cipher::Cipher;
pub use handshake_error::HandshakeError;
pub use initiator::Initiator;
pub use key_error::{KeyError, KeyProblem};
pub use key_pair::KeyPair;
pub use open_error::OpenError;
pub use private_key::PrivateKey;
pub use public_key::PublicKey;
pub use replay_window::{DEFAULT_REPLAY_WINDOW_PACKETS, MAX_REPLAY_WINDOW_PACKETS, ReplayWindow};
pub use responder::{Hello, read_hello};
pub use seal_error::SealError;
pub use server_keys::ServerKeys;
pub use transport::Transport;
