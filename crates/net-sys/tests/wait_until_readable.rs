//! `poll(2)` through its safe wrapper, on real UDP sockets.

use std::error::Error;
use std::net::{Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

use net_sys::{ReadinessSlot, wait_until_readable};

type TestResult = Result<(), Box<dyn Error>>;

fn local_socket() -> std::io::Result<UdpSocket> {
    UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
}

#[test]
fn times_out_when_nothing_arrives() -> TestResult {
    let socket = local_socket()?;
    let mut slots = [ReadinessSlot::new(&socket)];
    let started = Instant::now();
    assert_eq!(
        wait_until_readable(&mut slots, Duration::from_millis(50))?,
        0
    );
    assert!(started.elapsed() >= Duration::from_millis(40));
    assert!(slots.iter().all(|slot| !slot.is_ready()));
    Ok(())
}

#[test]
fn reports_exactly_the_socket_that_received_something() -> TestResult {
    let quiet = local_socket()?;
    let busy = local_socket()?;
    local_socket()?.send_to(b"hello", busy.local_addr()?)?;
    let mut slots = [ReadinessSlot::new(&quiet), ReadinessSlot::new(&busy)];
    assert_eq!(wait_until_readable(&mut slots, Duration::from_secs(5))?, 1);
    let ready: Vec<bool> = slots.iter().map(ReadinessSlot::is_ready).collect();
    assert_eq!(ready, [false, true]);
    Ok(())
}

#[test]
fn no_slots_is_a_plain_sleep() -> TestResult {
    assert_eq!(wait_until_readable(&mut [], Duration::from_millis(1))?, 0);
    Ok(())
}
