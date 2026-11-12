//! One human-readable line per sender event, for stderr.

use crate::srtla::SenderEvent;

/// Formats an event, e.g. `10.1.0.2: link established`.
#[must_use]
pub fn log_line(event: &SenderEvent) -> String {
    match event {
        SenderEvent::LinkAdded { source } => format!("{source}: link added"),
        SenderEvent::LinkRemoved { source } => format!("{source}: link removed"),
        SenderEvent::LinkNotOpened { source } => {
            format!("{source}: cannot bind a socket to this address (retrying)")
        }
        SenderEvent::GroupRegistered { source } => format!("{source}: group registered"),
        SenderEvent::GroupIdMismatch { source } => {
            format!("{source}: REG2 with someone else's group id, ignored")
        }
        SenderEvent::LinkEstablished { source } => format!("{source}: link established"),
        SenderEvent::LinkFailed { source } => {
            format!("{source}: link failed, registering it again")
        }
        SenderEvent::SendFailed { source } => format!("{source}: send failed, link disabled"),
        SenderEvent::NoLinkAvailable => "warning: no link available".to_owned(),
        SenderEvent::TryingReceiver { receiver } => format!("trying the receiver at {receiver}"),
        SenderEvent::GaveUp { had_connected } => if *had_connected {
            "could not re-establish any link, giving up"
        } else {
            "could not establish any link, giving up"
        }
        .to_owned(),
    }
}
