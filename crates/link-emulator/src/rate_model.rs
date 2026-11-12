//! Synthetic traces: a Markov chain over capacity levels, fitted on a real trace or written
//! by hand. A fitted model reproduces the statistics of the original without copying it.

use crate::random::Random;
use crate::{Trace, TraceError};

/// Capacity as a Markov chain over levels, one step per bin.
#[derive(Debug, Clone, PartialEq)]
pub struct RateModel {
    /// Bin length in milliseconds.
    pub bin_ms: u64,
    /// Delivery opportunities per bin for each level.
    pub levels: Vec<u32>,
    /// `transitions[i][j]`: probability of going from level `i` to level `j`.
    pub transitions: Vec<Vec<f64>>,
}

impl RateModel {
    /// A hand-written model: capacities in Mbps, and the mean time spent in each level.
    #[must_use]
    pub fn markov(bin_ms: u64, levels_mbps: &[f64], mean_dwell_ms: u64) -> Self {
        let levels = levels_mbps
            .iter()
            .map(|&mbps| per_bin(mbps, bin_ms))
            .collect();
        let n = levels_mbps.len();
        let stay = if mean_dwell_ms <= bin_ms {
            0.0
        } else {
            1.0 - to_f64(bin_ms) / to_f64(mean_dwell_ms)
        };
        let move_each = if n > 1 {
            (1.0 - stay) / to_f64(n as u64 - 1)
        } else {
            0.0
        };
        let transitions = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        if i == j {
                            if n > 1 { stay } else { 1.0 }
                        } else {
                            move_each
                        }
                    })
                    .collect()
            })
            .collect();
        Self {
            bin_ms,
            levels,
            transitions,
        }
    }

    /// Fits a model on one period of `trace`, with `level_count` quantile levels.
    #[must_use]
    pub fn fit(trace: &Trace, bin_ms: u64, level_count: usize) -> Self {
        let bin_ms = bin_ms.max(1);
        let bins = usize::try_from(trace.period_ms().div_ceil(bin_ms))
            .unwrap_or(1)
            .max(1);
        let mut counts = vec![0u32; bins];
        for index in 0..trace.len() {
            let bin = usize::try_from(trace.opportunity_ms(index).saturating_sub(1) / bin_ms)
                .unwrap_or(0);
            if let Some(count) = counts.get_mut(bin.min(bins - 1)) {
                *count += 1;
            }
        }
        let mut sorted = counts.clone();
        sorted.sort_unstable();
        let level_count = level_count.clamp(1, bins);
        let bucket = |count: u32| -> usize {
            let rank = sorted.partition_point(|&c| c < count);
            (rank * level_count / bins).min(level_count - 1)
        };
        let mut sums = vec![(0u64, 0u64); level_count];
        for &count in &counts {
            let entry = &mut sums[bucket(count)];
            entry.0 += u64::from(count);
            entry.1 += 1;
        }
        let levels = sums
            .iter()
            .map(|&(sum, n)| u32::try_from(sum.checked_div(n).unwrap_or(0)).unwrap_or(u32::MAX))
            .collect();
        let mut transitions = vec![vec![0.0; level_count]; level_count];
        for pair in counts.windows(2) {
            transitions[bucket(pair[0])][bucket(pair[1])] += 1.0;
        }
        for (i, row) in transitions.iter_mut().enumerate() {
            let total: f64 = row.iter().sum();
            if total == 0.0 {
                row[i] = 1.0;
            } else {
                for p in row.iter_mut() {
                    *p /= total;
                }
            }
        }
        Self {
            bin_ms,
            levels,
            transitions,
        }
    }

    /// Generates a trace of `duration_ms`, starting at level 0.
    ///
    /// # Errors
    /// Returns [`TraceError::Empty`] if the generated trace carries no opportunity.
    pub fn generate(&self, duration_ms: u64, seed: u64) -> Result<Trace, TraceError> {
        let mut rng = Random::new(seed);
        let mut level = 0usize;
        let mut opportunities = Vec::new();
        let mut start = 0u64;
        while start < duration_ms {
            let count = u64::from(self.levels.get(level).copied().unwrap_or(0));
            for k in 0..count {
                opportunities.push(start + 1 + k * self.bin_ms / count.max(1));
            }
            level = self.next_level(level, rng.next_unit());
            start += self.bin_ms;
        }
        if opportunities.last() != Some(&duration_ms) {
            // Close the period exactly at `duration_ms` so the trace loops on time.
            opportunities.push(duration_ms);
        }
        Trace::from_opportunities(opportunities)
    }

    fn next_level(&self, level: usize, draw: f64) -> usize {
        let Some(row) = self.transitions.get(level) else {
            return level;
        };
        let mut acc = 0.0;
        for (j, p) in row.iter().enumerate() {
            acc += p;
            if draw < acc {
                return j;
            }
        }
        level
    }
}

fn per_bin(mbps: f64, bin_ms: u64) -> u32 {
    let bytes = mbps * 1_000_000.0 / 8.0 * to_f64(bin_ms) / 1000.0;
    let opportunities = (bytes / f64::from(crate::OPPORTUNITY_BYTES)).round();
    if opportunities <= 0.0 {
        0
    } else if opportunities >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            opportunities as u32
        }
    }
}

#[allow(clippy::cast_precision_loss)]
fn to_f64(value: u64) -> f64 {
    value as f64
}
