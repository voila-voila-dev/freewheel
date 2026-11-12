//! Wire formats for link bonding: our own transport and SRTLA compatibility.
//!
//! Pure functions only: no sockets, no clocks, no allocation on the hot path.
#![no_std]

pub mod srtla;
