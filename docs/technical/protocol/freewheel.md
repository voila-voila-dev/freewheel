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

**What a link delivers only says what it was given.** On links faster than the stream, the
delivery rate measures the share the scheduler gave each link, not what it could carry: every
split is then stable, and a link that came back stayed at 25–45 % of the traffic (CI run
44179514166: 43 % median, 30 % on one run). So a link that has not filled its window for 1 s
counts at least the average rate of the usable links, for its drain time and its window; one
found full keeps its own estimate for a second. Two alike links then split evenly, and a weak
link still proves its limit by filling its window. 3 local runs: a restored link at 48–50 %
(against 30–46 %), real-ride 48–68 frames lost (baseline 49–70 in CI), trace-drive 525,
trace-city 0–3 (0 before).

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
a modem's queue, keeps a saturated queue under 700 ms, gives a restored link an even share back on fast links. Nothing allocates per packet (`freewheel_zero_allocations.rs`).

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
after it came back, against 40 % for irlserver and 13 % for the baseline. The next CI run gave
43 % against irlserver's 55 %: run to run, ours ranged 30–46 % and irlserver's 42–77 %. Ours was
stuck (the split above); irlserver overshoots, the link that comes back taking up to 3/4 of the
traffic of two alike links, which is no better. The gate now asks for 90 % of the even share
(45 % on `link-drop`'s two links) and at least the baseline's.

**Longest gap on `trace-drive`: 6.4 s against 3.8 s**, not gated. From 17 to 24 s and 83 to 87 s
the three traces together carry 0.4–2.4 Mbit/s for a 3.4 Mbit/s stream: no transport delivers
those seconds whole. The candidate spreads the shortfall, one or two packets late in almost
every frame, so nearly every frame is damaged for the length of the hole; the baseline damages
many more frames overall (1754) but lets an intact one through now and then, which cuts its
gaps short. The answer is lowering the encoder's bitrate when the links cannot carry it, not
the scheduler.

## Why frames are lost (2026-11-22)

Before adaptive FEC, each lost frame of the step 3 CI captures (run 44262198730, 3 runs) was
classified against the emulated network (`packet_capture::LossCauses`, second table of
`bench-transport`; method in `labs/srtla-capture/README.md`, "Transport bench"). Medians of the
runs, `transport → transport`:

| Scenario | Lost | Capacity | Recovery | Isolated loss | Link down, queue, late |
|---|---|---|---|---|---|
| trace-drive | 517 | 433 (427–433) | 89 (79–90) | 0 | 0 |
| real-ride | 61 | 61 (53–70) | 0 | 0 | 0 |
| all-links-drop | 186 | 182 | 4 | 0 | 0 |

Every frame is lost while the links together carry less than the stream (17 short seconds on
`trace-drive`, 4 on `real-ride`, 5 on `all-links-drop`), or in the 2–5 s after, while the stream
catches up (on `trace-drive`, every frame of 87–89 s, with 5–8 Mbit/s back for 3.5). The
random and bursty losses of `real-ride` and `burst-loss` cost nothing: SRT's own retransmissions
repair them within its 2 s. Upstream loses 1 frame that way on `real-ride`, irlserver 19; on
`trace-drive` upstream also loses 1012 frames to full modem queues, which our scheduler no longer fills.
Hence no FEC: it repairs isolated losses, there are none left, and it would spend capacity that is
missing exactly when frames are lost. Next is step 5, the encoder's bitrate following the links.

## Cross-layer bitrate and capacity probing (M1b step 5)

`freewheel-sender` reports what the links carry every 100 ms (`bitrate_control::CapacityReport`,
JSON on a Unix socket); the lab's encoder (`lab-encoder`) follows `bitrate_control::Controller`:
under pressure (1 % dropped at the box, or 20 % queuing), down to 80 % of the capacity and by
at least a fifth, unless the stream already fits there; once calm (2 s), straight to what the
measured capacity carries with that headroom, then +5 % every 500 ms beyond it. When links come
back, straight to what they carry at the first report. An outage of every link holds the
bitrate.

A link only measures what it is given: once the encoder is low, the links report that bitrate,
and the controller has nothing to climb back to (step 5's first CI run: 18 s from 0.3 to 2 Mbit/s
after `all-links-drop`, at +5–15 % a step). So the sender probes (`bonding_sender::freewheel`,
`probe.rs`):

- **When**: the encoder is below 90 % of its ceiling (median live traffic of the last second)
  and the usable links are not known to carry what the ceiling needs with the controller's
  headroom (`ControllerSettings::capacity_for_bps`: 4.46 Mbit/s for the lab's 3 Mbit/s video
  and 128 kbit/s audio). The ceiling comes from `FREEWHEEL_MAX_VIDEO_BPS` and
  `FREEWHEEL_AUDIO_BPS`; without them, no probing.
- **How**: on a link with spare room (not found full by the scheduler within 1 s), a 150 ms run
  of padding (data of the background class, 1 500-byte datagrams, acknowledged and dropped by
  the receiver) tops the link's traffic up to twice what it is known to carry, within the
  congestion window of that rate. The run's delivery rate is the receiver's byte count between
  the first ACK of its padding and the later ones (at least 50 ms apart). At most one run per
  link per second.
- **What it gives**: a probed rate, counted in the link's capacity for 10 s or until the
  scheduler finds the link full (then what it delivers is the measure). The scheduler itself
  still splits by delivery rate.
- **After a stall**, a link that comes back is probed at once at the rate it had.
- **Budget**: padding is at most 10 % of the estimated capacity on average (a bucket earning
  10 %, two seconds deep, kept through an outage), and a run starts only when the bucket holds
  all of it: a run cut short would measure less than the link carries.

Padding is the stand-in for the recording upload (step 6): the same class, the same spare
capacity, then doing useful work.

CI, 3 runs (run 45135774902), intact video against the baseline (`upstream/upstream`):
`trace-drive` 1906 kbit/s against 1093 (164 frames lost against 1717), `all-links-drop` 2190
against 2434 (186 frames lost against 209, longest gap 6.1 s against 6.9 s), `real-ride` 2801
against 2974 (18 frames lost against 45); the other scenarios equal. The gate still fails on the
last two:

- `all-links-drop`: the encoder is back at 3 Mbit/s 1.2 s after the links return (18 s before
  probing), but during the outage it went down to 0.3 Mbit/s. The frames of the outage's last
  seconds arrive, light; the baseline loses them at full size.
- `real-ride`: when two of three links are cut (92.7 s), the encoder steps down to 0.33 Mbit/s
  although the third link carries 2 Mbit/s, then climbs back by 99.4 s. Where the pressure comes
  from (SRT's catch-up on the surviving link?) is not yet measured.

What the reports and the encoded frames then showed, and what changed:

- **The lab's encoder was slow to climb.** x264's average bitrate mode (ABR) remembers the
  stream's past: set back to 3 Mbit/s after 6 s at 0.3, it made 2.2 Mbit/s a second later and
  3 Mbit/s only 10 s later. `lab-encoder` now holds its bitrate by the VBV buffer alone (capped
  CRF, `shim/x264_shim.c`): at a constant setting the same bitrate as the baseline's ffmpeg
  (3.12 against 3.06 Mbit/s over 20 s, the start's full buffer), and a new one followed within a
  frame or two, as the box's hardware encoder in constant bitrate does. Test
  `follows_a_higher_bitrate_within_a_second`.
- **The pressure after a cut is not the encoder's.** With two links cut, every byte sent on the
  third was late (overflow = sent) and the box dropped 1–17 Mbit/s: SRT retransmitting what the
  cut links lost. Lowering the picture does not relieve it, so the controller no longer lowers a
  stream that fits what the links are measured to carry, and climbs at once when links come back
  (both outages and partial cuts), without waiting for calm.
- **An outage of every link holds the bitrate** (decided on the bench, `decisions.md`): nothing
  measures the links while they are all down, and they come back at the rate they had. Lab, one
  run of `all-links-drop`: 132 frames lost and a 4.1 s gap at the held bitrate, against 195 and
  6.4 s at the minimum (baseline 209, 6.9 s); 2683 kbit/s intact against 2475 (baseline 2434).

## Alternative to evaluate

Multipath QUIC with datagrams (IETF draft): a standard, but a large dependency and less control over media-aware scheduling.

## How we decide

Same recorded traces and emulated links for every candidate, against SRTLA + SRT as baseline: `labs/srtla-capture/bench-transport.sh` + `bench-transport --gate <candidate>` (method in `labs/srtla-capture/README.md`, "Transport bench"; baseline in `bench/README.md`, "Transport").

| Metric | Target |
|---|---|
| Frames lost or late (live) | lower than baseline |
| Intact video bitrate (frames that arrived with the encoder's bytes) | at least the baseline's |
| Glass-to-glass latency, p50 / p99 | equal or lower |
| Recovery after a link dies | faster |
| Recording delivered after an outage | 100 % |
