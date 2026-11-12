//! Helpers shared by the integration tests.
#![allow(dead_code)]

use link_emulator::{
    Departure, Event, Fate, GilbertElliott, Link, LinkConfig, SendResult, Tower, Trace,
};

/// Packet size used by the tests.
pub const PACKET: u32 = 1200;

/// A link configuration with 30 ms propagation.
pub fn config(
    trace: Trace,
    queue_bytes: u32,
    loss: GilbertElliott,
    events: Vec<Event>,
) -> LinkConfig {
    LinkConfig {
        trace,
        propagation_ms: 30,
        queue_bytes,
        loss,
        events,
    }
}

/// A link with 30 ms propagation.
pub fn link(
    trace: Trace,
    queue_bytes: u32,
    loss: GilbertElliott,
    events: Vec<Event>,
    seed: u64,
) -> Link {
    Link::new(config(trace, queue_bytes, loss, events), seed)
}

/// A constant-rate trace with `per_ms` opportunities per millisecond.
pub fn constant(per_ms: u32) -> Trace {
    Trace::constant(per_ms, 1000).unwrap_or_else(|e| unreachable!("valid constant trace: {e}"))
}

/// Sends one packet every `interval_ms` for `duration_ms`, returns all departures.
pub fn run(
    link: &mut Link,
    interval_ms: u64,
    duration_ms: u64,
    mut tower: Option<&mut Tower>,
) -> (Vec<Departure>, usize) {
    let mut departures = Vec::new();
    let mut dropped = 0;
    let mut id = 0;
    for now in 0..duration_ms {
        departures.extend(link.advance(now, tower.as_deref_mut()));
        if now % interval_ms == 0 {
            if link.send(id, PACKET, now) == SendResult::QueueFull {
                dropped += 1;
            }
            id += 1;
        }
    }
    departures.extend(link.advance(duration_ms + 10_000, tower));
    (departures, dropped)
}

/// `(sent_ms, arrival_ms)` of delivered packets.
pub fn delivered(departures: &[Departure]) -> impl Iterator<Item = (u64, u64)> + '_ {
    departures.iter().filter_map(|d| match d.fate {
        Fate::Delivered { arrival_ms } => Some((d.sent_ms, arrival_ms)),
        Fate::Lost | Fate::Flushed => None,
    })
}
