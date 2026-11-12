//! A mark written by the lab when a scenario does something (cut a link, restart a process…).

/// A mark written by the lab: one line `<microseconds since the Unix epoch> <label>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    /// When, in microseconds since the Unix epoch.
    pub at_us: u64,
    /// What happened, e.g. `cut link 2 both`.
    pub label: String,
}

impl Mark {
    /// Parses a marks file; lines that do not start with a timestamp are skipped.
    #[must_use]
    pub fn parse_all(text: &str) -> Vec<Self> {
        text.lines()
            .filter_map(|line| {
                let (at_us, label) = line.split_once(' ')?;
                Some(Self {
                    at_us: at_us.parse().ok()?,
                    label: label.to_owned(),
                })
            })
            .collect()
    }
}
