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

## Alternative to evaluate

Multipath QUIC with datagrams (IETF draft): a standard, but a large dependency and less control over media-aware scheduling.

## How we decide

Same recorded traces and emulated links for every candidate, against SRTLA + SRT as baseline.

| Metric | Target |
|---|---|
| Frames lost or late (live) | lower than baseline |
| Average delivered bitrate | higher than baseline |
| Glass-to-glass latency, p50 / p99 | equal or lower |
| Recovery after a link dies | faster |
| Recording delivered after an outage | 100 % |
