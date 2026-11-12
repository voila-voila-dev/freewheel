//! Capacity traces in the Mahimahi format: one line per delivery opportunity,
//! each line the millisecond at which one MTU-sized packet can leave. The trace loops.

use crate::TraceError;

/// Bytes one delivery opportunity can carry (one MTU, as in Mahimahi).
pub const OPPORTUNITY_BYTES: u32 = 1500;

/// A looping sequence of delivery opportunities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    opportunities_ms: Vec<u64>,
    period_ms: u64,
}

impl Trace {
    /// Parses a Mahimahi trace. Blank lines are ignored.
    ///
    /// # Errors
    /// Returns a [`TraceError`] if the trace is empty, malformed or not sorted.
    pub fn parse(text: &str) -> Result<Self, TraceError> {
        let mut opportunities_ms = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let at: u64 = line
                .parse()
                .map_err(|_| TraceError::InvalidLine { line: index + 1 })?;
            if opportunities_ms
                .last()
                .is_some_and(|&previous| at < previous)
            {
                return Err(TraceError::NotSorted { line: index + 1 });
            }
            opportunities_ms.push(at);
        }
        Self::from_opportunities(opportunities_ms)
    }

    /// A trace with a constant rate: `per_ms` opportunities every millisecond for `period_ms`.
    ///
    /// # Errors
    /// Returns [`TraceError::Empty`] if `per_ms` or `period_ms` is zero.
    pub fn constant(per_ms: u32, period_ms: u64) -> Result<Self, TraceError> {
        let mut opportunities_ms = Vec::new();
        for at in 1..=period_ms {
            opportunities_ms.extend(std::iter::repeat_n(at, per_ms as usize));
        }
        Self::from_opportunities(opportunities_ms)
    }

    pub(crate) fn from_opportunities(opportunities_ms: Vec<u64>) -> Result<Self, TraceError> {
        let period_ms = match opportunities_ms.last() {
            Some(&last) if last > 0 => last,
            _ => return Err(TraceError::Empty),
        };
        Ok(Self {
            opportunities_ms,
            period_ms,
        })
    }

    /// Number of opportunities in one period.
    #[must_use]
    pub fn len(&self) -> usize {
        self.opportunities_ms.len()
    }

    /// A parsed trace always has at least one opportunity.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.opportunities_ms.is_empty()
    }

    /// Length of one period in milliseconds.
    #[must_use]
    pub fn period_ms(&self) -> u64 {
        self.period_ms
    }

    /// Time of the `index`-th opportunity since the start, looping over the period.
    #[must_use]
    pub fn opportunity_ms(&self, index: usize) -> u64 {
        let len = self.opportunities_ms.len();
        let lap = (index / len) as u64;
        self.opportunities_ms[index % len] + lap * self.period_ms
    }

    /// Average capacity in bits per second over one period.
    #[must_use]
    pub fn mean_bps(&self) -> u64 {
        self.len() as u64 * u64::from(OPPORTUNITY_BYTES) * 8 * 1000 / self.period_ms
    }
}
