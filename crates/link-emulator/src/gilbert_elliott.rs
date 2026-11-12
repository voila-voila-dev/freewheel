//! Bursty loss: the Gilbert-Elliott two-state model.

use crate::random::Random;

/// Two-state loss model: losses cluster in "bad" periods, as on real radio links.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GilbertElliott {
    /// Probability of moving from good to bad at each step.
    pub good_to_bad: f64,
    /// Probability of moving from bad to good at each step.
    pub bad_to_good: f64,
    /// Loss probability while good.
    pub loss_when_good: f64,
    /// Loss probability while bad.
    pub loss_when_bad: f64,
    bad: bool,
}

impl GilbertElliott {
    /// A model starting in the good state.
    #[must_use]
    pub const fn new(
        good_to_bad: f64,
        bad_to_good: f64,
        loss_when_good: f64,
        loss_when_bad: f64,
    ) -> Self {
        Self {
            good_to_bad,
            bad_to_good,
            loss_when_good,
            loss_when_bad,
            bad: false,
        }
    }

    /// No loss at all.
    #[must_use]
    pub const fn lossless() -> Self {
        Self::new(0.0, 1.0, 0.0, 0.0)
    }

    /// Independent loss with probability `p` (no bursts).
    #[must_use]
    pub const fn bernoulli(p: f64) -> Self {
        Self::new(0.0, 1.0, p, p)
    }

    /// Long-run share of steps spent in the bad state.
    #[must_use]
    pub fn bad_share(&self) -> f64 {
        let total = self.good_to_bad + self.bad_to_good;
        if total == 0.0 {
            0.0
        } else {
            self.good_to_bad / total
        }
    }

    pub(crate) fn step(&mut self, rng: &mut Random) {
        let flip = if self.bad {
            self.bad_to_good
        } else {
            self.good_to_bad
        };
        if rng.next_unit() < flip {
            self.bad = !self.bad;
        }
    }

    pub(crate) fn loss_probability(&self) -> f64 {
        if self.bad {
            self.loss_when_bad
        } else {
            self.loss_when_good
        }
    }
}
