//! What the transport tells the encoder, a few times a second: how much the links carry.
//!
//! On the wire between the two processes it is one JSON object per datagram (the Rust ↔
//! agent contract of `docs/technical/decisions.md`), e.g.
//! `{"capacity_bps":4200000,"sent_bps":3500000,"overflow_bps":0,"dropped_bps":0,"usable_links":3}`.

use core::fmt::{self, Write as _};

/// Longest encoded report, in bytes (five 20-digit numbers and the keys).
pub const MAX_REPORT_LEN: usize = 192;

/// What the transport measured over the last period. Rates are on the wire (headers included).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapacityReport {
    /// What the usable links together are estimated to carry, in bit/s.
    pub capacity_bps: u64,
    /// Live data sent on the links, in bit/s.
    pub sent_bps: u64,
    /// Of that, sent to arrive late (queuing in a modem), in bit/s.
    pub overflow_bps: u64,
    /// Live data dropped at the box because no link would deliver it in time, in bit/s.
    pub dropped_bps: u64,
    /// Links that may carry data now.
    pub usable_links: u8,
}

const KEYS: [&str; 5] = [
    "capacity_bps",
    "sent_bps",
    "overflow_bps",
    "dropped_bps",
    "usable_links",
];

impl CapacityReport {
    /// Writes the report as JSON into `out`; its length, or `None` if `out` is too short.
    #[must_use]
    pub fn encode(&self, out: &mut [u8]) -> Option<usize> {
        let mut writer = SliceWriter { out, len: 0 };
        write!(
            writer,
            "{{\"{}\":{},\"{}\":{},\"{}\":{},\"{}\":{},\"{}\":{}}}",
            KEYS[0],
            self.capacity_bps,
            KEYS[1],
            self.sent_bps,
            KEYS[2],
            self.overflow_bps,
            KEYS[3],
            self.dropped_bps,
            KEYS[4],
            self.usable_links
        )
        .ok()?;
        Some(writer.len)
    }

    /// Reads a report written by [`encode`](Self::encode). Every key must be there with an
    /// unsigned integer; anything else is `None`.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let text = core::str::from_utf8(bytes).ok()?.trim();
        let body = text.strip_prefix('{')?.strip_suffix('}')?;
        let mut values = [None::<u64>; KEYS.len()];
        for field in body.split(',') {
            let (key, value) = field.split_once(':')?;
            let key = key.trim().strip_prefix('"')?.strip_suffix('"')?;
            let index = KEYS.iter().position(|known| *known == key)?;
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            values[index] = Some(value.parse().ok()?);
        }
        Some(Self {
            capacity_bps: values[0]?,
            sent_bps: values[1]?,
            overflow_bps: values[2]?,
            dropped_bps: values[3]?,
            usable_links: u8::try_from(values[4]?).ok()?,
        })
    }
}

/// `core::fmt::Write` into a byte slice, failing when it is full.
struct SliceWriter<'a> {
    out: &'a mut [u8],
    len: usize,
}

impl fmt::Write for SliceWriter<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(fmt::Error)?;
        let slot = self.out.get_mut(self.len..end).ok_or(fmt::Error)?;
        slot.copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
