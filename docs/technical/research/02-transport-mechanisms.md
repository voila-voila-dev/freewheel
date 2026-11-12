# 02 — Transport mechanisms (the engine)

Research note, 2026-11-10. Angle: the mechanisms inside the freewheel transport (scheduling,
congestion control, estimation, loss recovery, receiver buffering). Competitors, media-aware
customization, security and embedded performance are covered by sibling notes.

Legend: **[V]** verified against a cited source; **[I]** inference or engineering judgement, to be
proven by the experiments below.

## Context

- Box: ROCK 5T, H.265 at 2–8 Mbps, 1–4 heterogeneous links (4G modems with local SIMs, Wi-Fi,
  phone hotspot), 10–15 W, long outages expected. Server: own VPS. Rust, no third-party crates
  except audited crypto (`docs/technical/decisions.md`).
- Goals in order (`docs/technical/protocol/freewheel.md`): never lose the recording; best picture; low,
  stable latency; survive link churn; secure by default. Three classes: Live (deadline),
  Reliable (ordered), Background (scavenger).
- Baseline: SRTLA (SRT inside, per-link window driven by SRTLA ACKs, link picked by lowest
  in-flight/window ratio). Everything below has to beat it on the same traces.
- `crates/link-emulator` today models i.i.d. loss plus uniform jitter per packet. It has **no
  rate, no queue, no burst loss, no outage, no correlation between links**. Most mechanisms
  below cannot be evaluated until that changes (see Experiments). **[V]** (read the code)

## Problem

The cellular uplink is the hardest part of the path, and the hardest part is not loss, it is
**delay**:

- Uplink capacity changes by 10x in seconds (cell load, fading, handover). A sender that
  overshoots does not see loss right away; it fills buffers. An IMC 2016 study measured up to
  ~4 s of extra latency from on-device uplink buffering, with a cellular firmware buffer holding
  up to 300 KB (49 % of end-to-end delay at 2 Mbps), and found that CoDel/PIE in the host do not
  fix it. **[V]** (Qian et al.) At 4 Mbps, 300 KB is 600 ms of video sitting in a modem we cannot see into. **[I]**
- Links are heterogeneous (Wi-Fi 20 ms, 4G 40–150 ms, hotspot via a phone with its own
  buffer), so naive per-packet spreading creates reordering and head-of-line blocking at the
  receiver.
- Losses are bursty and correlated: a handover or a tunnel kills one link for 0.1–10 s; two
  modems on the same tower degrade together. **[I]**
- Data is not free: a local SIM can be cheap, a roaming SIM is expensive, Wi-Fi is free.

## Needs (derived)

1. Keep the modem buffers nearly empty: the queue must live in our process, where the scheduler
   can still choose which link and which packet, and drop what is late.
2. Estimate per-link capacity and one-way delay continuously, including on idle or standby links.
3. Place every live packet where it arrives before its deadline with highest probability, at the
   lowest data cost.
4. Repair losses inside the latency budget, with a mix of FEC (no RTT wait) and ARQ (no
   overhead when clean), tuned from measured loss patterns.
5. Detect a dying link in less than one RTT-ish and move its in-flight data elsewhere.
6. Feed the encoder a capacity number it can trust (cross-layer with `bitrate-control`).

A key design fact **[I]**: the stream ends at YouTube/Twitch, whose own glass-to-glass latency is
several seconds. A transport latency budget of **800 ms to 2 s** costs the viewer almost nothing
and buys 3–10 cellular RTTs of retransmission time. So ARQ is the backbone and FEC is the
accelerator and the outage insurance, not the other way round. A "low latency" profile
(~300 ms) shifts the balance to FEC.

## Mechanisms

### 1. Multipath scheduling

| Scheduler | How | Fit | Cost |
|---|---|---|---|
| Round-robin / weighted (SRTLA-like) | Pick link with most free window | Simple, but ignores delay: fast + slow links → reordering | Trivial |
| minRTT (MPTCP default) | Lowest smoothed RTT with free window | Good when links are similar; overloads the fast link's queue and ignores queue already built | Trivial |
| ECF (Earliest Completion First) | Use the slow path only if waiting for the fast one is worse | Built for streaming over heterogeneous paths; authors report better path use **[V]** | Low |
| BLEST | Skip the slow path if it would cause HoL blocking at receiver | Targets reordering; best timeout figures in a survey's comparison **[V]** | Low |
| DAPS | Precomputed schedule from RTT ratios | Reacts slowly to change **[V]**: poor for cellular | Low |
| Redundant | Duplicate on all links | Best latency, N× cost; use per-class (audio, keyframe headers, control) | Trivial |
| Learning-based (RL, bandits) | Learn link choice from rewards | Opaque, data-hungry, hard to test deterministically; Chorus/Converge get their gains mostly from **coordination with the app** (QoE feedback, path-aware FEC), not from ML **[V]** for the results, **[I]** for the attribution | High |

**Recommendation [I]:** an **Earliest-Arrival-Time (EAT)** scheduler, the per-packet
generalisation of ECF/BLEST. For each link `l` and packet `p`:

```
eat(l, p) = now + backlog_bytes(l) / rate(l) + owd_p90(l) + size(p) / rate(l)
p_ok(l, p) = (1 - loss(l)) * P[eat <= deadline(p)]
score(l, p) = p_ok(l, p) - λ · cost_per_byte(l) · size(p)
```

Pick the max score; if the best `p_ok` is below a class threshold (audio 0.999, I-frame 0.99,
P-frame 0.95), also send a copy or a repair symbol on the second-best link. `backlog(l)` is the
per-link pacing queue in our process, so the formula sees the queue that minRTT ignores.
`owd_p90` instead of the mean makes the scheduler pessimistic about jittery links. This keeps
the fast link full first (minRTT behaviour), spills to slower links only when the fast queue
would make the packet late (ECF behaviour), and never sends a packet that would arrive after
the receiver's playout of later packets (BLEST behaviour). Complexity: O(links) per packet,
4 links: negligible.

### 2. Per-link congestion control

Each link gets its own controller (coupled CC across links, à la MPTCP LIA, protects other
flows at a shared bottleneck; our links rarely share one, so uncoupled is fine **[I]**).

| CC | Signal | Fit for cellular uplink video |
|---|---|---|
| Loss-based (Cubic, Reno) | Loss | Fills the 300 KB modem buffer before reacting: **no** |
| BBRv3 (draft-ietf-ccwg-bbr, Experimental, -06 July 2026) **[V]** | Delivery rate + min RTT, periodic probing | Good rate estimator; but ProbeBW up-cycles overshoot by 25 %, ProbeRTT drains, and the WG itself discussed adapting ProbeRTT for dynamically changing paths **[V]**; tuned for bulk, not app-limited video **[I]** |
| GCC (WebRTC) | One-way delay gradient (trendline), loss | Proven at scale for app-limited video; slow ramp-up; delay-gradient blind to standing queue **[I]** |
| SCReAMv2 (draft-ietf-ccwg-rfc8298bis, WG draft 2026) **[V]** | Queue delay (LEDBAT-like, no clock sync), loss, ECN/L4S; pacing and min rate **[V]** | Designed by Ericsson for video over cellular, handles L4S. Best starting point **[I]** |
| NADA | Delay + loss into one price | Similar family, less field use |
| Copa | Target rate from queuing delay, mode switch vs buffer-fillers | Low delay; competitive mode helps on Wi-Fi with others **[I]** |
| Sprout | Stochastic forecast of cellular delivery | Lowest delay but ~0.55 of ABC's utilisation on Mahimahi LTE traces **[V]** |
| Verus | Learned delay profile | ~0.72 utilisation, ~2x ABC's delay in the same study **[V]** |
| ABC / L4S-Prague | Explicit marks from the network | Best trade-off when available; requires RAN support. T-Mobile US announced L4S on 5G-Advanced (2025), 3GPP Rel-19 specifies per-flow UL/DL ECN marking **[V]**; for 4G local SIMs abroad, assume absent **[I]** |

**Recommendation [I]:** a **SCReAM-style delay-target controller** per link, with BBR-style
delivery-rate sampling as the capacity estimate:

- Control variable: per-link pacing rate (not a window): video is app-limited.
- Signal: queuing delay = `owd - owd_min(window 10–30 s)`, from receiver timestamps (no clock
  sync needed, only differences). Target 30–60 ms on 4G, 10–20 ms on Wi-Fi.
- Increase additively when queue < target and delivery rate ≈ send rate; multiplicative
  decrease scaled by how far over target; hard cut on loss bursts.
- `owd_min` expiry must be handover-aware: after a cell change, reset the min (the path really
  changed) instead of thinking the queue grew.
- Read ECN on the server (IP_RECVTOS via our own FFI crate; std does not expose it) and
  react Prague-style if CE marks appear. Cheap insurance for L4S networks.

### 3. Capacity estimation

- **Ack timing (primary).** Receiver sends compact feedback every ~20 ms or every N packets: per
  received packet (per-link seq, arrival time delta), like WebRTC transport-wide CC. Sender
  computes delivery rate = bytes acked / ack interval, over the last ~RTT. This is the BBR
  delivery-rate estimator, works only when the link is not app-limited, so flag samples. **[V]**
  for BBR's method, **[I]** for the adaptation.
- **Packet trains / chirps.** To estimate a standby or freshly-woken link without loading it:
  send a short chirp (e.g. 10 packets of 1200 B at increasing rates) every 10–30 s and read
  dispersion at the receiver. Cellular uplink scheduling (grant-based, TTI 1 ms) makes
  dispersion noisy: trains give a range, not a number **[I]**. Use them for "is this link worth
  waking up", not for steady-state control.
- **Modem metrics as predictors.** ModemManager's `Modem.Signal` interface exposes RSRP, RSRQ,
  SINR/SNR per technology, polled via `Setup(rate_s)` or event-driven via `SetupThresholds()`
  (≥1.20); key names vary by version **[V]**. ML studies predict LTE throughput from
  RSRP/RSRQ/SINR with R² ≈ 0.93 (downlink) **[V]**; uplink prediction from CQI/MCS/BLER exists only
  in recent 5G testbed preprints **[V]**. Realistic use **[I]**: not to compute capacity, but as a
  **prior and an early-warning**: SINR drop > X dB or RSRP below −110 dBm lowers the link's
  `p_ok` before the delay signal shows it; serving cell id change = handover hint. Poll at 1–2 s
  (modem polling costs power); the agent (Bun) reads D-Bus and pushes to Rust over the existing
  Unix socket.

### 4. Bufferbloat in modems and phones

- Root rule **[I]**: our pacer never lets more than `rate × target_delay` bytes sit below us
  (kernel qdisc + driver + modem). Set small `SO_SNDBUF`, pace at 1.0–1.25× estimated rate with
  bursts of ≤2–4 packets, and keep the real queue in a per-link userland queue where we can
  re-route or drop.
- The phone hotspot adds Wi-Fi + phone + modem buffers we do not control: treat it as a high-
  jitter link with a larger `owd_p90` and lower class eligibility (no audio-only reliance).
- On the box, `fq` or `cake` qdisc on modem interfaces is cheap insurance but not a fix
  (the IMC study says host AQM does not solve firmware buffering) **[V]**.

### 5. Handover and link death detection

Signals, fastest first **[I]**: (a) feedback silence > max(3 × feedback interval, owd_p99 −
owd_p50 + 50 ms); (b) queue delay jump > 3× target; (c) ModemManager state / cell id change /
registration loss; (d) NetworkManager interface down. State machine per link:

```
Probing → Warming → Active ⇄ Suspect → Dead → Probing
                       └→ Standby (cost policy) → Warming
```

On `Suspect`: no new primary data, unacked in-flight packets with remaining deadline budget are
**proactively re-sent** on the best other link (do not wait for NACK), FEC repair rate on other
links rises. Cellular handovers last tens to hundreds of ms; tunnels and dead zones much longer.

### 6. Pacing

Frames arrive in bursts (an I-frame can be 20–50 packets at once). Pace each link at its rate;
the scheduler spreads an I-frame across links by EAT, so a large frame naturally uses all
links in parallel (lower frame completion time). Linux `SO_TXTIME` + `fq` can offload pacing to
the kernel, via our FFI crate; a userland timer at 1 ms granularity is enough at 8 Mbps
(~700 pkt/s) **[I]**.

### 7. Forward error correction

| Code | Notes | Verdict |
|---|---|---|
| XOR row / 2D (SMPTE 2022-1 style) | Trivial, fixes single losses per row/column | Fallback for weak CPU; poor on bursts **[I]** |
| Reed-Solomon GF(2^8), block | MDS, well known, old enough that core patents are long expired **[I]** (check) | Good for fixed blocks (per frame); block latency = waiting for the block |
| Sliding-window RLC, RFC 8681 (Standards Track, 2020) **[V]** | Repair = random linear combination of the source symbols in a sliding window, over GF(2) or GF(2^8); lower FEC latency and better recovery than block codes for real-time **[V]** | **Primary choice [I]**. Simple to implement (Gaussian elimination on small windows); check its IPR disclosures before shipping |
| Tetrys, RFC 9407 (Experimental, IRTF, 2023) **[V]** | Elastic window of unacked sources, RTT-independent recovery **[V]** | Same math as RLC plus ack-driven window: maps exactly onto our feedback loop. Borrow the idea, not the wire format |
| RaptorQ, RFC 6330 | Excellent for large objects; Qualcomm IPR declared under RAND **with possible royalty/fee** **[V]** | **Avoid** for an MIT product; also overkill for small windows |

How we would use it **[I]**:

- One coding window per class, over global sequence numbers, window = last ~deadline budget of
  source packets (shrinks when receiver acks).
- **Cross-path coding:** repair symbols go preferentially on a *different* link than the
  sources they protect. A whole-link outage then turns into recoverable erasures. This is the
  main real gain over per-path FEC.
- Adaptive rate from a per-link Gilbert-Elliott fit (p, r, loss in bad state) every second:
  redundancy = expected losses in window + margin, capped by spare capacity. Zero repair on a
  clean link with comfortable ARQ budget; ramp up when budget is tight or link is `Suspect`.
- Cost: GF(2^8) RLC at 8 Mbps with 5–20 % repair and windows of ~100 symbols is a few hundred MB/s
  of table-lookup XOR at most; the RK3588 NEON can do it. To be measured.

### 8. Deadline-aware ARQ

- Receiver detects loss per link via per-link sequence numbers (like QUIC per-path packet
  numbers), which removes the cross-path reordering false-positive problem. It NACKs after a
  short per-link reorder tolerance (~1/8 RTT), not a global timer.
- Sender retransmits only if `now + owd_p90(best link) < deadline(p)` and the packet is still
  useful (not superseded, frame not already abandoned); always on the link with highest
  `p_ok`, often a different one. Retransmissions may be repair symbols instead of copies (one
  repair fixes any one loss in the window: better than a copy when several are missing).
- Reliable class: classic ARQ without deadline. Background class: ARQ at scavenger priority
  (LEDBAT-like: only when live queue is empty and the link's queue delay is under target/2).

### 9. Receiver: reordering and jitter buffer

- Reorder by global sequence; release by **media timestamp + fixed playout offset** (like SRT
  TSBPD). The offset is the latency budget; it changes only rarely (on profile change or after
  long-term statistics), never per packet, because the downstream SRT/RTMP output expects
  steady timing.
- Adaptive suggestion **[I]**: compute offset = p99 of (max link owd_p99 + one ARQ round on the
  best link) over the last minutes; apply changes at keyframe boundaries.
- Late packets are counted (telemetry) then dropped for live, but still delivered to the
  recording store if it is the same data (it is not: recording goes full quality in Background).

### 10. Standby links, warm-up after outages, data cost

- **Standby:** an expensive link sends a keepalive every 1–2 s (NAT binding, RTT) and a chirp
  every 30 s; ~1–3 KB/s. It becomes Active when the cheap links' aggregate `p_ok` for live
  drops, before frames are lost.
- **Shadow warm-up [I]:** a link back from an outage (or from standby) first carries only
  *redundant* traffic: FEC repair and duplicates. Its CC ramps up and its estimates converge
  while nothing depends on it. It is promoted to primary data when its OWD variance and loss
  settle. A failed warm-up costs bytes, not frames.
- **Cost-aware scheduling:** each link has `cost_per_GB` and an optional daily/monthly budget
  from the agent. λ in the score rises as budget burns. Background class never uses a link with
  cost > 0 unless allowed. Telemetry shows €/hour live.

### 11. MTU and header overhead

- Cellular paths often have MTU < 1500 (GTP/IPsec tunnels, IPv6 transition) **[I]**. Default
  datagram 1200 B (QUIC's guaranteed minimum), then per-link PLPMTUD (RFC 8899) probing up to
  ~1400 B. Never rely on ICMP.
- Overhead at 1200 B: IPv4 20 + UDP 8 + our header ~14 (type/flags 1, link id 1, per-link seq
  varint 2–4, global seq 4, timestamp 3–4) + AEAD tag 16 ≈ 58 B ≈ 4.8 %. SRTLA carries an SRT
  header inside (16 B) plus SRT's own ACK traffic; our native mode can drop that layer.
  Feedback packets from the server consume downlink only (usually cheap on cellular).

### 12. Alternatives considered

- **Multipath QUIC** (draft-ietf-quic-multipath-21, IESG-approved, in RFC Editor queue since
  2026-03 **[V]**): standard per-path packet numbers and path management are worth copying as
  ideas. As a dependency it breaks "zero crates", and its reliable-stream model fights deadline
  dropping. Datagrams + our own scheduler would mean reimplementing most of this note anyway **[I]**.

## Proposed engine architecture

Pure core (`no_std`-friendly, no I/O, driven by `(now, event)` → actions), adapters for sockets.
Proposed modules in `bonding-protocol` / future `bonding-sender`, `bonding-receiver`:

| Module | Role | Timescale |
|---|---|---|
| `wire` | Packet formats: data, repair, feedback, probe, control | — |
| `link` | Per-link state machine (Probing/Warming/Active/Suspect/Standby/Dead) | events |
| `estimator` | OWD min/p50/p90/p99, delivery rate, Gilbert-Elliott loss, ECN | per feedback (~20 ms) |
| `cc` | Delay-target pacing-rate controller per link | per feedback |
| `pacer` | Per-link userland queue + token bucket | per packet (~1 ms) |
| `scheduler` | EAT + class thresholds + cost; duplicates; proactive re-send on Suspect | per packet |
| `fec` | RLC GF(2^8) encoder/decoder, sliding window, cross-path placement, adaptive rate | per packet / 1 s rate |
| `arq` | Deadline-aware NACK handling (sender) and NACK generation (receiver) | per loss |
| `playout` | Reorder + TSBPD-style release, offset policy | per packet / minutes |
| `capacity` | Aggregate usable rate for live = Σ rate × availability, minus FEC/ARQ headroom → `bitrate-control` | 100 ms–1 s |
| `hints` | Modem signal / cell id / interface events from agent → link priors | 1–2 s |
| `budget` | Cost policy, λ, standby decisions | 10 s–hours |

Control loops, nested from fast to slow: pacer (ms) → CC (tens of ms) → scheduler shares and
FEC rate (100 ms–1 s) → encoder bitrate (0.5–2 s, with hysteresis so FEC and encoder do not
fight) → latency budget and cost policy (minutes). Each faster loop must converge before the
slower one acts on it **[I]**.

## Experiments to run

Prerequisite: **extend `link-emulator`** into a trace-driven bottleneck model (still seeded and
deterministic):
- delivery opportunities per ms (Mahimahi trace format, so public LTE traces work as-is **[V]**
  ABC/Sprout used them), finite FIFO with configurable size (100–300 KB "modem"), Gilbert-Elliott
  loss, scripted outages and handover stalls, rate steps, and a shared "tower" factor to
  correlate two links.
- Real traces: on the bike, log per-link feedback (owd, delivery rate, loss) passively all day,
  plus a short saturating probe every N minutes on cheap links, and ModemManager signal + cell id
  + GPS, all into the telemetry outbox. Replay into the emulator.

| # | Question | Setup | Metric |
|---|---|---|---|
| E1 | Scheduler | SRTLA-like vs minRTT vs ECF vs EAT, 2–4 heterogeneous links | late/lost frames, p99 latency, delivered bitrate |
| E2 | CC | fixed window vs GCC-like vs SCReAM-like vs BBR-like, 300 KB buffer | queue delay p50/p99, utilisation |
| E3 | Recovery mix | ARQ only / FEC only / both, budgets 300 ms, 800 ms, 2 s | lost frames vs overhead bytes |
| E4 | Cross-path FEC | per-path vs cross-path repair under single-link outages 0.2–5 s | frames lost during outage |
| E5 | Death detection | feedback-silence thresholds, proactive re-send on/off | frames lost per link death, false positives/h |
| E6 | Warm-up | direct promotion vs shadow warm-up | frames lost in 30 s after link return |
| E7 | Modem hints | lead time between SINR/RSRP drop and delay/loss onset on real traces | correlation, ms of warning; then A/B with hints on |
| E8 | Cost | cheap+expensive links, budget policy | €/hour vs lost frames Pareto curve |
| E9 | MTU | 1200 vs PLPMTUD | overhead %, black-hole incidents |
| E10 | CPU | RLC encode/decode on RK3588 at 8 Mbps | % of one A55/A76 core, mW |

All runs: same seeds, same traces, SRTLA + SRT baseline in the table, results in CI as a
regression bench.

## Open questions

- IPR status of RFC 8681 RLC and Tetrys: any disclosures on the datatracker? (Not checked here.)
- How much do two modems on the same operator/tower correlate? Is a second SIM on another
  operator worth more than a second modem? Needs field data.
- Does any operator on the route expose L4S/ECN on uplink? Unknown outside T-Mobile US.
- Can the modem's own buffer be shrunk (AT commands, QMI/MBIM settings)? Vendor-specific.
- Uplink-specific value of RSRP/SINR as predictors: published evidence is mostly downlink.
- Latency budget default: 1 s? Ask what the IRL viewer flow (chat interaction) tolerates.
- Clock: OWD uses differences only, but long sessions need drift handling (ppm skew over hours).

## Sources

- RFC 8681, Sliding Window RLC FEC: https://www.rfc-editor.org/rfc/rfc8681.html
- RFC 9407, Tetrys: https://www.rfc-editor.org/rfc/rfc9407.html
- RFC 6330, RaptorQ: https://rfc-editor.org/rfc/rfc6330 ; Qualcomm IPR: https://datatracker.ietf.org/ipr/1958 , https://datatracker.ietf.org/ipr/1292/ ; https://en.wikipedia.org/wiki/Raptor_code
- SCReAMv2 drafts: https://datatracker.ietf.org/doc/html/draft-ietf-ccwg-rfc8298bis-screamv2-00 , https://www.ietf.org/archive/id/draft-johansson-ccwg-rfc8298bis-screamv2-07.html
- BBRv3 draft: https://www.ietf.org/archive/id/draft-ietf-ccwg-bbr-06.txt ; IETF 123 CCWG minutes: https://ekr.github.io/auto-minutes/minutes/ietf123/ccwg.txt ; BBRv3 eval (IFIP 2025): https://dl.ifip.org/db/conf/networking/networking2025/1571125683.pdf
- Multipath QUIC: https://datatracker.ietf.org/doc/draft-ietf-quic-multipath/history/
- ECF: https://research.ibm.com/publications/ecf-an-mptcp-path-scheduler-to-manage-heterogeneous-paths ; scheduler survey: https://arxiv.org/pdf/2309.09372 ; schedulers on Wi-Fi/cellular video: https://core.ac.uk/download/482463701.pdf
- Converge (SIGCOMM 2023): https://experts.colorado.edu/display/pubid_360080
- Chorus (MobiCom 2024): https://greenlv.github.io/files/Chorus_MobiCom24.pdf
- ABC (NSDI 2020, Mahimahi LTE comparison incl. Sprout, Verus, Copa): https://arxiv.org/pdf/1905.03429
- Sprout: https://www.cs.princeton.edu/courses/archive/fall16/cos561/papers/Sprout13.pdf ; Verus: https://sites.nyuad.nyu.edu/dtl/projects/verus/index.html ; C2TCP: https://arxiv.org/pdf/1807.02689
- Uplink on-device bufferbloat (IMC 2016): https://www.ics.uci.edu/~alfchen/pubs/yihua_imc16.pdf
- L4S on 5G: https://www.mobileworldlive.com/t-mobile-us/t-mobile-us-unleashes-l4s-on-5g-advanced/ ; https://blog.viavisolutions.com/2025/11/04/validating-l4s-implementations-for-5g-advanced/ ; 3GPP Rel-19 CR: https://www.3gpp.org/ftp/tsg_sa/wg2_arch/TSGS2_169_Fukuoka_2025-05/INBOX/DRAFTS/Rel-19-XRM_Ph2/Draft2-S2-2505744.docx
- ModemManager Signal interface: https://distributions.freedesktop.org/software/ModemManager/api/latest/gdbus-org.freedesktop.ModemManager1.Modem.Signal.html
- Throughput prediction from radio metrics: https://networking.umn.edu/lumos5g ; https://arxiv.org/pdf/2604.09632
- RFC 8899 (PLPMTUD): https://www.rfc-editor.org/rfc/rfc8899
