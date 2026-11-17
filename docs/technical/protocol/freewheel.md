# freewheel transport (draft)

Our own protocol from the box to our server. SRTLA stays as a compatibility mode
(`docs/technical/protocol/srtla.md`) and as the baseline every benchmark must beat.

## Goals, in order

1. **Never lose the recording.** Lossy live is acceptable; the full-quality recording reaches the server eventually.
2. **Best picture for the network we have.** Use every link's capacity, all the time.
3. **Low, stable latency.** Recover losses without waiting a round trip when possible.
4. **Survive link churn.** SIM swaps, IP changes, NAT rebinding, a link dying mid-frame.
5. **Secure by default.** Authenticated and encrypted, no open relay.

## One pipe, three traffic classes

| Class | What | Delivery |
|---|---|---|
| Live | Encoded audio and video frames | Deadline-aware: late is useless, so drop or recover before the deadline |
| Reliable | Telemetry, control, GPS outbox | Ordered, never lost, small |
| Background | Full-quality recording upload | Scavenger: only spare capacity, resumes across outages |

## Candidate mechanisms (to benchmark, not decided)

- **Per-link estimation**: RTT, loss, jitter and capacity per link, from acknowledgement timing.
- **Scheduler by expected arrival time**: send each packet on the link where it arrives first, weighted by loss.
- **Media-aware priorities**: audio and keyframes duplicated across links; under pressure drop the least useful frames, never random packets.
- **Adaptive forward error correction**: repair packets sized to the measured loss, so most losses are fixed without a retransmission (sliding-window codes first; check patents before any fountain code).
- **Deadline-aware retransmission**: retransmit only what can still arrive in time, on the fastest link.
- **Cross-layer bitrate**: the transport gives `bitrate-control` the real aggregate capacity, instead of the encoder guessing from losses.
- **Session ids, not addresses**: links are identified by a session, so an IP change is a non-event.
- **Crypto**: Noise handshake + AEAD, from an audited crate. We never implement cryptography ourselves.

## Wire format v0

Implemented in `crates/bonding-protocol/src/freewheel/` (pure, `no_std`, no allocation).
Big-endian everywhere. A datagram is at most 1 500 bytes. v0 has no encryption yet: Noise
(M1b step 7) will wrap everything after the header.

### Header (12 bytes, every packet)

| Bytes | Field | Notes |
|---|---|---|
| 0 | magic `0xF0` | `F` for freewheel, `0` for the version. SRT data starts with a top bit of 0, SRT control with `0x80`–`0x8F`, SRTLA with `0x90`–`0x92`: the server tells the three protocols apart on one port from this byte |
| 1 | kind | below |
| 2 | link id | the link the packet travels on, or the one an ACK reports |
| 3 | traffic class | data only: `0x01` live, `0x02` reliable, `0x03` background. Must be 0 on every other kind |
| 4–11 | session id | 64 bits; links belong to a session, not to an address, so an IP change is a non-event |

### Kinds

| Kind | Name | Length | After the header |
|---|---|---|---|
| `0x01` | Data | 20 + payload (≤ 1 480) | sequence (32 bits, per session, shared by all links and classes, wrapping), send time (µs on the sender's clock, wrapping), payload |
| `0x02` | ACK | 28 + 8 × ranges (≤ 184) | largest sequence received on that link, its send time echoed, how long the receiver held it (µs), bytes received on that link (wrapping); then ranges of received sequences, first and last included |
| `0x03` | Keepalive | 16 | send time; answered by an ACK echoing it, so an idle link stays measured |
| `0x04` | Join | 12 | a link asks to join the session |
| `0x05` | Accept | 12 | the server accepts the link |
| `0x06` | Close | 13 | reason: `0x00` done, `0x01` refused, `0x02` timed out |

RTT of a link = now − echoed send time − ACK delay. Capacity comes from the received bytes
between two ACKs. Times are differences only, so the clocks need not agree.

### Strict decoding

Anything v0 does not define is refused, never ignored: unknown kind, class or close reason,
a non-zero class byte outside data, fixed-length kinds of another length, ACK ranges that are
not whole pairs. So every packet that decodes encodes back to the same bytes, which the tests
prove the M1 way (`tests/freewheel_*.rs`): round trips of random packets, exact layouts,
every input up to 2 bytes, every kind × class byte at every boundary length, 2 million random
mutations in release. Benchmark `freewheel_decode` keeps the decode allocation-free.

## First candidate (M1b step 3)

`freewheel-sender` and `freewheel-receiver` (crates `bonding-sender` and `bonding-receiver`,
modules `freewheel`), same command lines as the SRTLA programs. Like SRTLA, the candidate carries
the SRT stream unchanged, as live-class data, so the bench compares transports with SRT on top
for everyone. In the lab: `SENDER=transport RECEIVER=transport`.

**Session.** The box draws a random session id. Each link sends `Join` every 250 ms until the
server answers `Accept`. A packet from an unknown session or link gets `Close(refused)` and the
link joins again, which also covers a server restart. The server follows a link's address on
every packet. SRT's answers go back through the link that delivered last.

**Acknowledgements.** The server acknowledges each link every 4 data packets, or 5 ms after a
lone one, and answers a keepalive at once. Links idle for 50 ms send a keepalive, so every link
keeps a fresh RTT, including one that carries nothing.

**Estimation, per link, from each ACK.**
- RTT = now − echoed send time − ACK delay; smoothed (gain 1/8) and its minimum.
- Delivery rate: bytes the server received over at least 100 ms; the maximum of the last
  10 samples (1 s), never below 500 kbit/s.
- Loss: bytes acknowledged out of flight against bytes received, smoothed.
- Congestion window: twice the bandwidth-delay product (rate × minimum RTT), at least ten packets.

**Scheduling by expected arrival.** Each packet goes to the usable link where it should arrive
first: half the smoothed RTT plus the time to drain what that link has in flight at its rate,
stretched by its loss. Links within their window are preferred; when all are full, the earliest
anyway (live video cannot wait), but only if it should arrive within 500 ms.

**Late is dropped, not queued.** A live packet whose best expected arrival is over 1 s (half of
SRT's 2 s latency, so one retransmission still fits) is dropped at the box. The first version
queued it anyway: on `trace-drive` the modems' queues then held seconds of stale video after
each outage, and the stream stayed broken ~12 s after the capacity came back.

**A full modem queue looks like clock drift to SRT.** SRT's receiver measures its clock drift on
the delay of the caller's ACKACKs, and shifts its whole delivery schedule when that delay grows.
Queuing up to 1 s beyond the windows on `trace-drive` kept RTTs at 400–600 ms while capacity was
enough; SRT's buffer grew from 1970 to 2030 ms, and the latency of every frame with it (+26 ms
p50 against the baseline in CI). Hence the 500 ms bound above for packets sent beyond the windows
(4 local runs: p50 2113 → 2097 ms, frames lost 505 → 532, delivered 2900 → 2885 kbit/s; at
300 ms, p50 2093 but 646 frames lost). Sending ACKACKs on every link so the shortest queue sets
the drift was tried and changed nothing: all three queues were full.

**Stall and return.** A link that heard no ACK for 4 smoothed RTTs (at least 300 ms, at most
1 s) while it sent something stops carrying data and forgets its RTT, but keeps its rate history. Its
keepalives continue every 50 ms; the first ACK brings it back with its previous share at once.
This replaces SRTLA's 4 s timeout and its slow window ramp.

Tested on a simulated network (`crates/bonding-sender/tests/freewheel_transport.rs`): it prefers
the link that arrives first, spreads a stream no single link can carry, moves off a link whose
capacity collapses, leaves a dead link and takes it back within 200 ms, keeps late video out of
a modem's queue, keeps a saturated queue under 700 ms. Nothing allocates per packet (`freewheel_zero_allocations.rs`).

First lab results (2026-11-17, one local run on OrbStack, against the local baseline runs):

| Scenario | Frames lost: baseline → candidate | Delivered | Latency p50 / p99 |
|---|---|---|---|
| trace-drive | 1766 → **513** (irlserver 1903) | 1869 → **2902** kbit/s | 2083 / 2119 → 2129 / 2154 ms |
| real-ride | 70 → **45** (longest gap 825 → 363 ms) | 3287 → 3302 kbit/s | 2126 / 2157 → 2093 / 2117 ms |
| all-links-drop | 211 → **187** (gap 6.9 → 6.2 s) | 2686 → 2758 kbit/s | same |
| link-drop, burst-loss, varying-bandwidth, trace-city | 0–2 → 0–5 | same | same |

CI, 3 runs (2026-11-17, run 44124314452): the gain holds. trace-drive 1754 → **532** frames
lost, 1784 → **2886** kbit/s; real-ride 63 → 49; all-links-drop 209 → 186; trace-city and
varying-bandwidth 0 → 0 (the local misses were noise). `--gate` failed on one measure only:
trace-drive p50 2078 → 2104 ms (+26, slack 20), the drift above.

After the 500 ms bound (CI run 44151066706, 3 runs): trace-drive 1743 → **497** frames lost,
1832 → **2903** kbit/s, p50 2078 → 2096 ms (+18, within the slack). real-ride 58 → 59: both lose
the same seconds when two of three links are cut (90–93 s, 2 Mbit/s left for 3.4), 49 to 61
frames from run to run; the gate now allows +5 % or +3 frames per scenario, the total must still
be lower (742 against 2010). A restored link (`link-drop`) carries **48 %** of the traffic 15–25 s
after it came back, against 40 % for irlserver and 13 % for the baseline: the gate holds it there.

**Longest gap on `trace-drive`: 6.4 s against 3.8 s**, not gated. From 17 to 24 s and 83 to 87 s
the three traces together carry 0.4–2.4 Mbit/s for a 3.4 Mbit/s stream: no transport delivers
those seconds whole. The candidate spreads the shortfall, one or two packets late in almost
every frame, so nearly every frame is damaged for the length of the hole; the baseline damages
many more frames overall (1754) but lets an intact one through now and then, which cuts its
gaps short. The answer is lowering the encoder's bitrate when the links cannot carry it, not
the scheduler.

## Alternative to evaluate

Multipath QUIC with datagrams (IETF draft): a standard, but a large dependency and less control over media-aware scheduling.

## How we decide

Same recorded traces and emulated links for every candidate, against SRTLA + SRT as baseline: `labs/srtla-capture/bench-transport.sh` + `bench-transport --gate <candidate>` (method in `labs/srtla-capture/README.md`, "Transport bench"; baseline in `bench/README.md`, "Transport").

| Metric | Target |
|---|---|
| Frames lost or late (live) | lower than baseline |
| Average delivered bitrate | higher than baseline |
| Glass-to-glass latency, p50 / p99 | equal or lower |
| Recovery after a link dies | faster |
| Recording delivered after an outage | 100 % |
