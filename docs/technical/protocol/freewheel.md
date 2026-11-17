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
