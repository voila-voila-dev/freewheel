//! One direction of an emulated cellular link: trace-driven capacity, modem buffer,
//! propagation delay, bursty loss and scripted events.

use crate::random::Random;
use crate::{Departure, Event, Fate, LinkConfig, OPPORTUNITY_BYTES, SendResult, Tower};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
struct Queued {
    id: u64,
    bytes: u32,
    sent_ms: u64,
}

/// One direction of an emulated cellular link.
#[derive(Debug, Clone)]
pub struct Link {
    config: LinkConfig,
    rng: Random,
    queue: VecDeque<Queued>,
    queued_bytes: u32,
    next_opportunity: usize,
    next_event: usize,
    address: u32,
}

impl Link {
    /// Creates a link at time 0.
    #[must_use]
    pub fn new(mut config: LinkConfig, seed: u64) -> Self {
        config.events.sort_by_key(|event| event.starts_at());
        Self {
            config,
            rng: Random::new(seed),
            queue: VecDeque::new(),
            queued_bytes: 0,
            next_opportunity: 0,
            next_event: 0,
            address: 0,
        }
    }

    /// Hands a packet to the modem at `now_ms`. Call [`Link::advance`] up to `now_ms` first.
    pub fn send(&mut self, id: u64, bytes: u32, now_ms: u64) -> SendResult {
        if self.queued_bytes.saturating_add(bytes) > self.config.queue_bytes {
            return SendResult::QueueFull;
        }
        self.queued_bytes += bytes;
        self.queue.push_back(Queued {
            id,
            bytes,
            sent_ms: now_ms,
        });
        SendResult::Queued
    }

    /// Bytes currently waiting in the modem buffer.
    #[must_use]
    pub fn queued_bytes(&self) -> u32 {
        self.queued_bytes
    }

    /// Address generation: increases at every address change or modem reboot.
    #[must_use]
    pub fn address(&self) -> u32 {
        self.address
    }

    /// Runs the link up to and including `now_ms`, returning packets that left the modem.
    pub fn advance(&mut self, now_ms: u64, mut tower: Option<&mut Tower>) -> Vec<Departure> {
        let mut departures = Vec::new();
        loop {
            let at_ms = self.config.trace.opportunity_ms(self.next_opportunity);
            if at_ms > now_ms {
                break;
            }
            self.apply_events_until(at_ms, &mut departures);
            self.next_opportunity += 1;
            let share = self
                .config
                .events
                .iter()
                .map(|event| event.capacity_share(at_ms))
                .fold(1.0, f64::min);
            if share <= 0.0 || (share < 1.0 && self.rng.next_unit() >= share) {
                continue;
            }
            let extra = tower.as_deref_mut().map_or(0.0, |t| t.extra_loss(at_ms));
            self.serve_opportunity(at_ms, extra, &mut departures);
        }
        self.apply_events_until(now_ms, &mut departures);
        departures
    }

    fn apply_events_until(&mut self, at_ms: u64, out: &mut Vec<Departure>) {
        while let Some(&event) = self.config.events.get(self.next_event) {
            if event.starts_at() > at_ms {
                break;
            }
            self.next_event += 1;
            if let Event::ModemReboot { at_ms: start, .. } = event {
                self.flush(start, out);
            }
            if event.changes_address() {
                self.address += 1;
            }
        }
    }

    fn flush(&mut self, at_ms: u64, out: &mut Vec<Departure>) {
        let address = self.address;
        out.extend(self.queue.drain(..).map(|q| Departure {
            id: q.id,
            bytes: q.bytes,
            sent_ms: q.sent_ms,
            served_ms: at_ms,
            address,
            fate: Fate::Flushed,
        }));
        self.queued_bytes = 0;
    }

    fn serve_opportunity(&mut self, at_ms: u64, extra_loss: f64, out: &mut Vec<Departure>) {
        let mut budget = OPPORTUNITY_BYTES;
        while let Some(head) = self.queue.front().copied() {
            if head.bytes > budget || head.sent_ms > at_ms {
                break;
            }
            budget -= head.bytes;
            self.queue.pop_front();
            self.queued_bytes -= head.bytes;
            self.config.loss.step(&mut self.rng);
            let loss = (self.config.loss.loss_probability() + extra_loss).min(1.0);
            let fate = if self.rng.next_unit() < loss {
                Fate::Lost
            } else {
                Fate::Delivered {
                    arrival_ms: at_ms + u64::from(self.config.propagation_ms),
                }
            };
            out.push(Departure {
                id: head.id,
                bytes: head.bytes,
                sent_ms: head.sent_ms,
                served_ms: at_ms,
                address: self.address,
                fate,
            });
        }
    }
}
