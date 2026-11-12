//! Why a trace could not be parsed.

use std::fmt;

/// Why a trace could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceError {
    /// The trace has no opportunity.
    Empty,
    /// A line is not a non-negative integer.
    InvalidLine {
        /// 1-based line number.
        line: usize,
    },
    /// Timestamps go backwards.
    NotSorted {
        /// 1-based line number.
        line: usize,
    },
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "trace has no delivery opportunity"),
            Self::InvalidLine { line } => write!(f, "line {line} is not a millisecond timestamp"),
            Self::NotSorted { line } => write!(f, "line {line} goes back in time"),
        }
    }
}

impl std::error::Error for TraceError {}
