//! `SIGHUP` caught instead of killing the process.

use std::error::Error;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use net_sys::{catch_hangup, hangup_flag};

#[test]
fn a_hangup_sets_the_flag_instead_of_terminating() -> Result<(), Box<dyn Error>> {
    catch_hangup()?;
    assert!(!hangup_flag().swap(false, Ordering::Relaxed));
    let status = Command::new("kill")
        .args(["-HUP", &std::process::id().to_string()])
        .status()?;
    assert!(status.success());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !hangup_flag().load(Ordering::Relaxed) {
        assert!(Instant::now() < deadline, "SIGHUP never arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(hangup_flag().swap(false, Ordering::Relaxed));
    assert!(!hangup_flag().load(Ordering::Relaxed), "consumed once");
    Ok(())
}
