//! A cell tower shared by several links, so they degrade together.

use crate::GilbertElliott;
use crate::random::Random;

/// Shared state of a cell tower: links attached to the same tower degrade together.
#[derive(Debug, Clone)]
pub struct Tower {
    congestion: GilbertElliott,
    rng: Random,
    stepped_until_ms: u64,
}

impl Tower {
    /// A tower whose congestion follows `congestion`; its loss probabilities are extra
    /// loss added to every attached link while the tower is in that state.
    #[must_use]
    pub fn new(congestion: GilbertElliott, seed: u64) -> Self {
        Self {
            congestion,
            rng: Random::new(seed),
            stepped_until_ms: 0,
        }
    }

    /// Extra loss probability at `at_ms`. Steps the congestion process once per millisecond.
    pub fn extra_loss(&mut self, at_ms: u64) -> f64 {
        while self.stepped_until_ms < at_ms {
            self.congestion.step(&mut self.rng);
            self.stepped_until_ms += 1;
        }
        self.congestion.loss_probability()
    }
}
