//! What the lab's emulated network let each link's uplink carry, read back from its marks.
//!
//! The lab marks every `tc netem` setting it applies (`labs/srtla-capture/lib/impairments.sh`,
//! `shape_link`, and each step of a trace replay, `lib/traces.sh`) as
//! `netem link <k> <up|down|both>: <netem arguments>`. Only the uplink (box → server) matters
//! here: it is where the video goes.

use std::collections::BTreeMap;

use crate::Mark;

/// Prefix of the marks this reads.
pub const NETEM_MARK_PREFIX: &str = "netem link ";
const LOSS_EVERYTHING: &str = "100%";
const BITS_PER_KILOBIT: u64 = 1_000;
const BITS_PER_MEGABIT: u64 = 1_000_000;
const BITS_PER_GIGABIT: u64 = 1_000_000_000;

/// One `netem` setting of a link's uplink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UplinkSetting {
    /// Rate limit, in bit/s; `None` when the link is not limited (the lab's veth pairs).
    pub rate_bps: Option<u64>,
    /// Everything sent is lost: a cut link, a dead second of a trace.
    pub dead: bool,
    /// Random or bursty loss below 100 % (`loss 2%`, `loss gemodel …`).
    pub lossy: bool,
}

impl UplinkSetting {
    /// Reads `netem` arguments, e.g. `delay 30ms 10ms rate 6mbit limit 100 loss 0.5%`.
    #[must_use]
    pub fn parse(netem: &str) -> Self {
        let words: Vec<&str> = netem.split_whitespace().collect();
        let mut setting = Self::default();
        for (index, word) in words.iter().enumerate() {
            let next = words.get(index + 1).copied().unwrap_or_default();
            match *word {
                "rate" => setting.rate_bps = parse_rate_bps(next),
                "loss" if next == LOSS_EVERYTHING => setting.dead = true,
                "loss" => setting.lossy = true,
                _ => {}
            }
        }
        setting
    }

    /// What the uplink can carry, in bit/s: 0 when dead, infinite when not limited.
    #[must_use]
    pub fn capacity_bps(&self) -> f64 {
        if self.dead {
            0.0
        } else {
            self.rate_bps.map_or(f64::INFINITY, to_f64)
        }
    }
}

/// Every link's uplink settings over a run, in the order the lab applied them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmulatedUplinks {
    /// Link number (`k` of `10.<k>.0.2`) → `(microseconds since the Unix epoch, setting)`.
    links: BTreeMap<u8, Vec<(u64, UplinkSetting)>>,
}

impl EmulatedUplinks {
    /// Reads the `netem link …` marks; others are ignored.
    #[must_use]
    pub fn from_marks(marks: &[Mark]) -> Self {
        let mut uplinks = Self::default();
        for mark in marks {
            let Some(rest) = mark.label.strip_prefix(NETEM_MARK_PREFIX) else {
                continue;
            };
            let Some((target, netem)) = rest.split_once(": ") else {
                continue;
            };
            let Some((link, direction)) = target.split_once(' ') else {
                continue;
            };
            let (Ok(link), "up" | "both") = (link.parse::<u8>(), direction) else {
                continue;
            };
            uplinks
                .links
                .entry(link)
                .or_default()
                .push((mark.at_us, UplinkSetting::parse(netem)));
        }
        for settings in uplinks.links.values_mut() {
            settings.sort_by_key(|(at_us, _)| *at_us);
        }
        uplinks
    }

    /// No `netem` marks: a run from before the lab wrote them.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Link `link`'s setting at `at_us` (not limited, no loss, before its first mark).
    #[must_use]
    pub fn setting_at(&self, link: u8, at_us: u64) -> UplinkSetting {
        self.links
            .get(&link)
            .and_then(|settings| settings.iter().rev().find(|(from_us, _)| *from_us <= at_us))
            .map(|(_, setting)| *setting)
            .unwrap_or_default()
    }

    /// What all the uplinks together could carry on average between `from_us` and `to_us`,
    /// in bit/s (infinite as soon as one of them is not limited).
    #[must_use]
    pub fn capacity_bps_between(&self, from_us: u64, to_us: u64) -> f64 {
        if to_us <= from_us {
            return 0.0;
        }
        let span = to_f64(to_us - from_us);
        // A segment of zero length carries nothing, even on an unlimited link (∞ × 0).
        let share = |setting: UplinkSetting, from: u64, to: u64| {
            if to > from {
                setting.capacity_bps() * to_f64(to - from) / span
            } else {
                0.0
            }
        };
        self.links
            .values()
            .map(|settings| {
                let mut carried_bps = 0.0;
                let mut cursor = from_us;
                let mut current = settings
                    .iter()
                    .rev()
                    .find(|(at_us, _)| *at_us <= from_us)
                    .map(|(_, setting)| *setting)
                    .unwrap_or_default();
                for (at_us, setting) in settings
                    .iter()
                    .filter(|(at_us, _)| *at_us > from_us && *at_us < to_us)
                {
                    carried_bps += share(current, cursor, *at_us);
                    cursor = *at_us;
                    current = *setting;
                }
                carried_bps + share(current, cursor, to_us)
            })
            .sum()
    }
}

/// `6mbit`, `500kbit`, `2400kbit`, `1gbit`, `8000bit` → bit/s.
fn parse_rate_bps(text: &str) -> Option<u64> {
    let digits_end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(digits_end);
    let number: u64 = number.parse().ok()?;
    let per_unit = match unit {
        "bit" => 1,
        "kbit" => BITS_PER_KILOBIT,
        "mbit" => BITS_PER_MEGABIT,
        "gbit" => BITS_PER_GIGABIT,
        _ => return None,
    };
    number.checked_mul(per_unit)
}

#[allow(clippy::cast_precision_loss)]
const fn to_f64(value: u64) -> f64 {
    value as f64
}
