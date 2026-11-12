//! What the SRT listener behind the receiver saw: the video that made it, or not.

/// Receive counters of the SRT listener, summed over its restarts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SrtStatistics {
    /// Data packets received.
    pub received_packets: u64,
    /// Gaps in sequence numbers detected (most are filled later by reordering or retransmission).
    pub lost_packets: u64,
    /// Packets given up on because they arrived too late: lost video.
    pub dropped_packets: u64,
    /// Retransmitted packets received.
    pub retransmitted_packets: u64,
}

impl SrtStatistics {
    /// Parses `srt-live-transmit -pf:json -f` output: one JSON object per line, with running
    /// totals that start again from zero when the listener restarts.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut finished = Self::default();
        let mut current = Self::default();
        for line in text.lines() {
            let Some(receive) = line.split_once("\"recv\":{").map(|(_, rest)| rest) else {
                continue;
            };
            let next = Self {
                received_packets: field(receive, "packets"),
                lost_packets: field(receive, "packetsLost"),
                dropped_packets: field(receive, "packetsDropped"),
                retransmitted_packets: field(receive, "packetsRetransmitted"),
            };
            if next.received_packets < current.received_packets {
                finished = finished.plus(current);
            }
            current = next;
        }
        finished.plus(current)
    }

    /// Share of received packets that were dropped, in parts per million.
    #[must_use]
    pub fn dropped_ppm(&self) -> u64 {
        (self.dropped_packets * 1_000_000)
            .checked_div(self.received_packets + self.dropped_packets)
            .unwrap_or(0)
    }

    const fn plus(self, other: Self) -> Self {
        Self {
            received_packets: self.received_packets + other.received_packets,
            lost_packets: self.lost_packets + other.lost_packets,
            dropped_packets: self.dropped_packets + other.dropped_packets,
            retransmitted_packets: self.retransmitted_packets + other.retransmitted_packets,
        }
    }
}

/// Reads `"name":<integer>` exactly (so `packets` does not match `packetsLost`).
fn field(json: &str, name: &str) -> u64 {
    let key = format!("\"{name}\":");
    json.split_once(&key)
        .map(|(_, rest)| {
            rest.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(0)
}
