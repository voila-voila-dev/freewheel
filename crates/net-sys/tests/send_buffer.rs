//! `setsockopt(SO_SNDBUF)` through its safe wrapper.

use std::error::Error;
use std::net::{Ipv4Addr, UdpSocket};

use net_sys::set_send_buffer_bytes;

#[test]
fn a_large_send_buffer_is_accepted() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    set_send_buffer_bytes(&socket, 8 * 1024 * 1024)
        .or_else(|_| set_send_buffer_bytes(&socket, 1024 * 1024))?;
    Ok(())
}

#[test]
fn a_size_beyond_a_c_int_is_refused() -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
    assert!(set_send_buffer_bytes(&socket, usize::MAX).is_err());
    Ok(())
}
