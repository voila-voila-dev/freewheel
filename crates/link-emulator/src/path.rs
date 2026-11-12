//! One SIM: uplink and downlink sharing the same radio, events, power and data billing.

use crate::{DataMeter, Departure, Event, Fate, Link, LinkConfig, RadioPower, SendResult, Tower};

/// One SIM: an uplink and a downlink that share events, power and data billing.
#[derive(Debug, Clone)]
pub struct Path {
    /// Box → server.
    pub up: Link,
    /// Server → box (acknowledgements, control).
    pub down: Link,
    power: RadioPower,
    meter: DataMeter,
    activity_ms: Vec<u64>,
    last_activity_ms: Option<u64>,
    integrated_until_ms: u64,
    energy_uj: u64,
}

impl Path {
    /// Builds a path; `events` apply to both directions.
    #[must_use]
    pub fn new(
        mut up: LinkConfig,
        mut down: LinkConfig,
        events: &[Event],
        power: RadioPower,
        seed: u64,
    ) -> Self {
        up.events.extend_from_slice(events);
        down.events.extend_from_slice(events);
        Self {
            up: Link::new(up, seed),
            down: Link::new(down, seed ^ 0xD0_57),
            power,
            meter: DataMeter::default(),
            activity_ms: Vec::new(),
            last_activity_ms: None,
            integrated_until_ms: 0,
            energy_uj: 0,
        }
    }

    /// Sends a packet from the box.
    pub fn send_up(&mut self, id: u64, bytes: u32, now_ms: u64) -> SendResult {
        self.activity_ms.push(now_ms);
        self.up.send(id, bytes, now_ms)
    }

    /// Sends a packet to the box.
    pub fn send_down(&mut self, id: u64, bytes: u32, now_ms: u64) -> SendResult {
        self.down.send(id, bytes, now_ms)
    }

    /// Runs both directions up to `now_ms`. Returns `(uplink, downlink)` departures.
    pub fn advance(
        &mut self,
        now_ms: u64,
        mut tower: Option<&mut Tower>,
    ) -> (Vec<Departure>, Vec<Departure>) {
        let up = self.up.advance(now_ms, tower.as_deref_mut());
        let down = self.down.advance(now_ms, tower);
        self.meter.count(&up, true);
        self.meter.count(&down, false);
        self.activity_ms.extend(
            up.iter()
                .chain(&down)
                .filter(|d| d.fate != Fate::Flushed)
                .map(|d| d.served_ms),
        );
        self.integrate_energy(now_ms);
        (up, down)
    }

    /// Data billed so far.
    #[must_use]
    pub fn meter(&self) -> DataMeter {
        self.meter
    }

    /// Radio energy used so far, in millijoules.
    #[must_use]
    pub fn energy_mj(&self) -> u64 {
        self.energy_uj / 1_000
    }

    fn integrate_energy(&mut self, now_ms: u64) {
        self.activity_ms.sort_unstable();
        self.activity_ms.dedup();
        let mut activity = self.activity_ms.iter().copied().peekable();
        for ms in self.integrated_until_ms..=now_ms {
            while activity.next_if(|&at| at < ms).is_some() {}
            let active = activity.next_if_eq(&ms).is_some();
            if active {
                self.last_activity_ms = Some(ms);
            }
            let mw = if active {
                self.power.active_mw
            } else if self
                .last_activity_ms
                .is_some_and(|last| ms - last < self.power.tail_ms)
            {
                self.power.tail_mw
            } else {
                self.power.idle_mw
            };
            // 1 mW during 1 ms = 1 µJ.
            self.energy_uj += u64::from(mw);
        }
        self.activity_ms.retain(|&at| at > now_ms);
        self.integrated_until_ms = now_ms + 1;
    }
}
