//! One human-readable line per receiver event, for stderr.

use crate::srtla::ReceiverEvent;

/// Formats an event, e.g. `10.1.0.2:40000: group 3 registered`.
#[must_use]
pub fn log_line(event: &ReceiverEvent) -> String {
    match event {
        ReceiverEvent::GroupRegistered { group, from } => {
            format!("{from}: group {} registered", group.get())
        }
        ReceiverEvent::GroupRefused { from, reason } => {
            format!("{from}: group registration refused ({reason:?})")
        }
        ReceiverEvent::LinkRegistered { group, link, again } => format!(
            "{link} (group {}): link registered{}",
            group.get(),
            if *again { " again" } else { "" }
        ),
        ReceiverEvent::LinkRefused { link, reason } => {
            format!("{link}: link registration refused ({reason:?})")
        }
        ReceiverEvent::LinkTimedOut { group, link } => {
            format!("{link} (group {}): link removed (timed out)", group.get())
        }
        ReceiverEvent::GroupRemoved { group, reason } => {
            format!("group {}: removed ({reason:?})", group.get())
        }
        ReceiverEvent::SendToLinkFailed { link } => format!("{link}: send failed"),
    }
}
