//! Outages, handovers, address changes and modem reboots.

mod support;

use link_emulator::*;
use support::*;

#[test]
fn outages_hold_packets_until_service_returns() {
    let outage = Event::Outage {
        start_ms: 1_000,
        end_ms: 3_000,
    };
    let mut link = link(
        constant(1),
        1_000_000,
        GilbertElliott::lossless(),
        vec![outage],
        1,
    );
    let (departures, _) = run(&mut link, 10, 4_000, None);
    assert!(delivered(&departures).all(|(_, arrival)| !(1_031..3_000).contains(&arrival)));
    assert!(
        delivered(&departures)
            .any(|(sent, arrival)| (1_000..3_000).contains(&sent) && arrival >= 3_000)
    );
}

#[test]
fn handover_interrupts_then_ramps_capacity_back() {
    let handover = Event::Handover {
        at_ms: 1_000,
        interruption_ms: 100,
        ramp_ms: 1_000,
    };
    let mut link = link(
        constant(1),
        10_000_000,
        GilbertElliott::lossless(),
        vec![handover],
        3,
    );
    // Offer more than capacity so every opportunity is used.
    let mut served_per_100ms = [0u32; 30];
    let mut id = 0;
    for now in 0..3_000 {
        for d in link.advance(now, None) {
            if let Some(slot) =
                served_per_100ms.get_mut(usize::try_from(d.served_ms / 100).unwrap_or(0))
            {
                *slot += 1;
            }
        }
        for _ in 0..2 {
            link.send(id, PACKET, now);
            id += 1;
        }
    }
    assert_eq!(served_per_100ms[10], 0, "interruption");
    assert!(served_per_100ms[12] < served_per_100ms[19], "ramping up");
    assert_eq!(served_per_100ms[25], 100, "full capacity after the ramp");
}

#[test]
fn address_changes_are_visible_on_departures() {
    let change = Event::AddressChange { at_ms: 500 };
    let mut link = link(
        constant(1),
        1_000_000,
        GilbertElliott::lossless(),
        vec![change],
        1,
    );
    let (departures, _) = run(&mut link, 10, 1_000, None);
    assert!(
        departures
            .iter()
            .filter(|d| d.served_ms < 500)
            .all(|d| d.address == 0)
    );
    assert!(
        departures
            .iter()
            .filter(|d| d.served_ms >= 500)
            .all(|d| d.address == 1)
    );
    assert_eq!(link.address(), 1);
}

#[test]
fn modem_reboot_flushes_the_queue_and_changes_address() {
    // Saturate the link so packets are queued when the modem reboots.
    let reboot = Event::ModemReboot {
        at_ms: 1_000,
        down_ms: 2_000,
    };
    let mut link = link(
        constant(1),
        10_000_000,
        GilbertElliott::lossless(),
        vec![reboot],
        1,
    );
    let mut departures = Vec::new();
    for now in 0..4_000 {
        departures.extend(link.advance(now, None));
        link.send(now * 2, PACKET, now);
        link.send(now * 2 + 1, PACKET, now);
    }
    assert!(
        departures
            .iter()
            .any(|d| d.fate == Fate::Flushed && d.served_ms == 1_000)
    );
    assert!(
        departures
            .iter()
            .filter(|d| d.fate != Fate::Flushed)
            .all(|d| !(1_000..3_000).contains(&d.served_ms))
    );
    assert_eq!(link.address(), 1);
}
