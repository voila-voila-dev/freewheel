//! Why a setting read from the environment was refused.

use core::fmt;

/// A setting read from the environment that the sender cannot use: its variable, its value,
/// and what it should be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingError {
    /// The environment variable.
    pub variable: &'static str,
    /// What it held.
    pub value: String,
    /// What it should hold, e.g. "a whole number from 1 to 100".
    pub expected: String,
}

impl fmt::Display for SettingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}={}: expected {}",
            self.variable, self.value, self.expected
        )
    }
}

impl std::error::Error for SettingError {}
