//! Modem buffer, delay, bursty and correlated loss.

mod support;

use link_emulator::*;
use support::*;

#[test]
fn under_capacity_adds_only_propagation_delay() {
    let mut link = link(constant(1), 64_000, GilbertElliott::lossless(), vec![], 1);
    let (departures, dropped) = run(&mut link, 2, 5_000, None);
    assert_eq!(dropped, 0);
    assert!(delivered(&departures).all(|(sent, arrival)| arrival - sent <= 31));
}

#[test]
fn over_capacity_fills_the_modem_buffer_then_drops() {
    // 6 Mbps link (one opportunity every 2 ms), 9.6 Mbps offered.
    let every_2ms = (1..=500)
        .map(|i| (i * 2).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let trace = Trace::parse(&every_2ms).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(trace.mean_bps(), 6_000_000);
    let mut link = link(trace, 300_000, GilbertElliott::lossless(), vec![], 1);
    let (departures, dropped) = run(&mut link, 1, 5_000, None);
    assert!(dropped > 0, "the buffer must overflow");
    let worst = delivered(&departures)
        .map(|(s, a)| a - s)
        .max()
        .unwrap_or(0);
    assert!(
        worst > 300,
        "queueing delay should reach hundreds of ms, got {worst}"
    );
}

#[test]
fn gilbert_elliott_losses_come_in_bursts() {
    fn mean_burst(loss: GilbertElliott) -> f64 {
        let mut link = link(constant(4), 10_000_000, loss, vec![], 9);
        let (departures, _) = run(&mut link, 1, 50_000, None);
        let mut bursts = Vec::new();
        let mut current = 0u32;
        for departure in &departures {
            if departure.fate == Fate::Lost {
                current += 1;
            } else if current > 0 {
                bursts.push(current);
                current = 0;
            }
        }
        f64::from(bursts.iter().sum::<u32>())
            / f64::from(u32::try_from(bursts.len().max(1)).unwrap_or(1))
    }
    let bursty = mean_burst(GilbertElliott::new(0.01, 0.2, 0.0, 0.5));
    let random = mean_burst(GilbertElliott::bernoulli(0.024));
    assert!(
        bursty > 1.5 * random,
        "bursty {bursty:.2} vs random {random:.2}"
    );
}

#[test]
fn links_on_the_same_tower_lose_together() {
    fn lost_bins(seed: u64, tower: &mut Tower) -> std::collections::BTreeSet<u64> {
        let mut link = link(
            constant(1),
            1_000_000,
            GilbertElliott::bernoulli(0.001),
            vec![],
            seed,
        );
        let (departures, _) = run(&mut link, 1, 20_000, Some(tower));
        departures
            .iter()
            .filter(|d| d.fate == Fate::Lost)
            .map(|d| d.sent_ms / 50)
            .collect()
    }
    let congestion = GilbertElliott::new(0.0005, 0.01, 0.0, 0.6);
    let a = lost_bins(1, &mut Tower::new(congestion, 7));
    let b = lost_bins(2, &mut Tower::new(congestion, 7));
    let c = lost_bins(2, &mut Tower::new(congestion, 8));
    let together = a.intersection(&b).count();
    let apart = a.intersection(&c).count();
    assert!(
        together > 2 * apart.max(1),
        "same tower {together} vs different {apart}"
    );
}
