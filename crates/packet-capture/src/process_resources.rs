//! What a process cost: CPU time and peak memory, as the lab marks them at the end of a run.

const PREFIX: &str = "resources ";
const CPU_FIELD: &str = "cpu_ms=";
const PEAK_FIELD: &str = "peak_kb=";

/// CPU time and peak resident memory of the sender or the receiver, since it started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessResources {
    /// User + system CPU time, in milliseconds.
    pub cpu_ms: u64,
    /// Peak resident memory (`VmHWM`), in kilobytes.
    pub peak_kb: u64,
}

impl ProcessResources {
    /// Parses a mark `resources <role> cpu_ms=<n> peak_kb=<n>` for `role` (`sender`, `receiver`).
    #[must_use]
    pub fn parse(label: &str, role: &str) -> Option<Self> {
        let rest = label.strip_prefix(PREFIX)?.strip_prefix(role)?;
        let mut resources = Self::default();
        let mut found = (false, false);
        for word in rest.split_whitespace() {
            if let Some(value) = word.strip_prefix(CPU_FIELD) {
                resources.cpu_ms = value.parse().ok()?;
                found.0 = true;
            } else if let Some(value) = word.strip_prefix(PEAK_FIELD) {
                resources.peak_kb = value.parse().ok()?;
                found.1 = true;
            }
        }
        (found.0 && found.1).then_some(resources)
    }
}
