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
Big-endian everywhere. A datagram is at most 1 500 bytes. v0 is in clear: since M1b step 7 it
runs only behind `FREEWHEEL_PLAINTEXT=1`, and v1 (below) seals it.

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

### Background payload

Live and reliable payloads are opaque. A background payload (M1b step 6) starts with a kind byte
of its own (`bonding_protocol::freewheel::BackgroundPayload`):

| Kind | Name | Length | After the kind byte |
|---|---|---|---|
| `0x00` | Padding | 1 – 1 480 | zeros only: capacity probing's filler, nothing to keep |
| `0x01` | Recording chunk | 10 – 1 480 | offset of the first byte in the recording (64 bits), then 1 – 1 471 bytes of it |

The offset makes a resend idempotent: the server writes the bytes where they belong, whichever
copy arrives. Step 5's padding (1 480 zero bytes) is already padding. Decoding is as strict as
the packet's: an unknown kind, padding that is not all zero, a chunk without bytes or shorter
than its header are refused, and the encoder refuses to write them. Same proofs
(`tests/background_payload.rs`, `tests/background_fuzz.rs`): exact layouts, round trips,
every input up to 2 bytes, every kind at every boundary length, 2 million mutations in release;
benchmark `recording_chunk_decode` keeps it allocation-free.

The packet layer still treats data payloads as opaque, so a receiver without a recording counts,
acknowledges and drops background data as before, whatever it holds. With a recording, ACKs
carry the ranges of the recording chunks received (below, "Recording upload").

## Wire format v1 (M1b step 7)

v0 sealed: every packet authenticated, every payload encrypted (goal 5). Framing in
`bonding_protocol::freewheel` (`Envelope`, `frame_sealed`; pure, `no_std`); everything that
touches a key in `crates/freewheel-noise`. Encrypted by default, both sides; a side in one mode
drops the other's packets unanswered.

### Sealed packet (every packet after the handshake)

| Bytes | Field | Notes |
|---|---|---|
| 0 | magic `0xF1` | `F` for freewheel, `1` for the version; SRT, SRTLA and v0 told apart on this byte as before |
| 1–11 | kind, link id, class, session id | as v0, in clear: the server routes on them before decrypting |
| 12–19 | nonce | 64 bits, one counter per session and direction, shared by every link, from 0 after each handshake |
| 20 – n−17 | the v0 packet after its header, encrypted | |
| n−16 – n−1 | tag | 16 bytes |

Bytes 0–19 are the AEAD's associated data: an altered kind, link, class, session id or nonce
fails the tag like an altered payload. The AEAD is Noise's `CipherState` with associated data:
ChaCha20-Poly1305 (default) or AES-256-GCM (`FREEWHEEL_CIPHER`), nonce = 32 zero bits and the
counter (little-endian for ChaChaPoly, big-endian for AESGCM, Noise §12). 24 bytes per packet:
the largest payload goes from 1 480 to 1 456 bytes, the largest recording chunk from 1 471 to
1 447 (the sender clamps `CHUNK_BYTES`). A sealed packet's length follows from its kind's v0
length (+ 24): anything else is refused before any decryption.

### Handshake: Noise `IK`, one round trip

| Kind | Name | Length | After the header (class 0) |
|---|---|---|---|
| `0x07` | Hello | 116 | IK's first message (`e, es, s, ss`): the box's ephemeral key, its static key encrypted, the payload encrypted (104 bytes) |
| `0x08` | Welcome | 60 | IK's second (`e, ee, se`): the server's ephemeral key, an empty payload's tag (48 bytes) |

`Noise_IK_25519_ChaChaPoly_BLAKE2s` (or `_AESGCM_`), through `snow`; prologue `freewheel v1` +
the session id, so a hello replayed under another session id fails. The hello's payload is its
handshake number in the session (64 bits, from 1). The box knows the server's static public key
(`FREEWHEEL_SERVER_PUBLIC_KEY`), so the session opens in the round trip Join / Accept took in v0:

- **Box.** The first housekeeping starts the handshake; the same hello goes on every open link,
  and again unchanged every 250 ms until a welcome comes (any welcome on any link opens the
  session; each welcome accepts its link). A welcome that does not authenticate changes nothing.
  Links that join later (or whose hello was lost) send a sealed Join, answered by a sealed
  Accept: only the session's holder can add links. If nothing from the server has opened for
  3 s, the box starts a new handshake (number + 1): the server may have lost the session (a
  restart), and it cannot say so in a way the box could trust.
- **Server.** A hello that does not authenticate, or from a key not on
  `FREEWHEEL_AUTHORIZED_KEYS`: dropped unanswered, no state kept. A session id belongs to the box
  key that opened it: another key's hello for it is refused. A copy of the session's current
  hello (another link, a retry after a lost welcome) is answered with the same welcome, cached,
  and its link registered; a hello with a higher number opens the session again with new keys
  (the links stay); a lower or equal one is a replay, dropped. Each side keeps the previous keys
  until a packet opens with the new ones, so what was in flight still opens.
- **Replay window.** Per session and direction, a bitmap over the latest nonces (8 192 by
  default, 500 ms at 196 Mbit/s of full datagrams; `FREEWHEEL_REPLAY_WINDOW_PACKETS` on the
  server), allocated once: a nonce below the window or seen already is dropped and counted. Links
  reorder, so nonces may come in any order within it. Checked before the tag, marked after.
- **Costs.** A forged sealed packet costs one AEAD check; a forged hello two X25519 operations
  (IK's static key is encrypted, so the box is known only after them). Nothing allocates per
  packet; a handshake allocates (`snow`).
- **Not done.** Rekeying within a session (a later step, `roadmap.md`); keys are read once, at
  start.

Keys: one static pair per box, one for the server, made by `freewheel-keygen PATH` (the private
key in `PATH`, mode 0600, the public key in `PATH.pub` and printed; `--force` to replace one).
A private key others may read is refused. Resetting a box: a new pair, its new public key in the
server's list, the old one out. The authorised list: one key per line, a name after it allowed,
`#` comments.

Tests: `bonding-protocol` `tests/freewheel_envelope.rs` (layouts, lengths per kind, framing) and
`tests/envelope_fuzz.rs` (the v0 method: every input up to 2 bytes, every kind × class at every
boundary length, 2 million mutations in release); `freewheel-noise` `tests/handshake.rs`,
`transport.rs`, `replay_window.rs`, `key_files.rs`, `zero_allocations.rs`; `bonding-sender`
`tests/freewheel_encryption.rs` on the simulated network (one round trip, a lost hello on one
link, lost welcomes, an unknown box, a wrong server key, mixed modes, another box claiming the
session, replayed and altered packets, reordering across links, the recording byte for byte, a
server restart, an outage longer than the re-handshake), `freewheel_zero_allocations.rs`
encrypted; `freewheel_recording_off.rs` still proves the clear wire is step 5's to the bit.

**CPU** (benchmarks `freewheel_encrypt_chacha20poly1305`, `freewheel_encrypt_aes256gcm`: one
full datagram sealed then opened, measured by `bun run bench` in instructions per packet; the
ROCK 5T's numbers come in M3). On an Apple M-series laptop, 2026-11-24: ChaCha20-Poly1305
7.3 µs per datagram (both ends), AES-256-GCM 24 µs. On aarch64 the RustCrypto crates use the CPU's
instructions only when built with `--cfg aes_armv8 --cfg polyval_armv8 --cfg chacha20_force_neon`:
then 6.3 µs and 2.2 µs. Not set yet: whether the box builds with them goes with M3's choice of the
default cipher. At the lab's 3 Mbit/s live and 6 Mbit/s recording (~900 datagrams/s), ChaCha costs
each side about 0.3 % of a core.

**Lab, one local run** (2026-11-24, encrypted, recording on; indicative, CI decides):
`all-links-drop` 2518 kbit/s intact, 189 frames lost, recording whole 5 s after the stream (step 6,
CI: 2538, 186); `link-drop` 3035 kbit/s, 1 frame lost (3056); `receiver-restart` 163 frames lost,
a 5.2 s gap, the same in clear (163, 5.3 s): the box opens the session again 3 s into the
silence, while the server is back after 1.3 s.

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
at least a fifth, unless links were just cut and the stream fits what the others carry; once calm (2 s), straight to what the
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
  cut links lost. Lowering the picture does not relieve it, so after a cut (fewer usable links
  than in the last calm window) the controller no longer lowers a stream that fits what the
  others are measured to carry, and it climbs at once when links come back (both outages and
  partial cuts), without waiting for calm. Only after a cut: on links that stay, the measured
  capacity lags a drop (`trace-drive`, 43.4 s: 5.7 Mbit/s reported for ~3.4 carried), and
  holding there lost 432 frames in a lab run against 172 with the pressure as the measure.
- **An outage of every link holds the bitrate** (decided on the bench, `decisions.md`): nothing
  measures the links while they are all down, and they come back at the rate they had. Lab, one
  run of `all-links-drop`: 132 frames lost and a 4.1 s gap at the held bitrate, against 195 and
  6.4 s at the minimum (baseline 209, 6.9 s); 2683 kbit/s intact against 2475 (baseline 2434).

CI run 45173685823 (3 runs, medians): every scenario beat the baseline but `real-ride`, 2891
kbit/s intact against 2970 (16 frames lost against 49). Its reports showed why:

- **The links that stay are measured low at a cut.** They carried little before it, and the
  transport probes them up: at the cut of link 1 (20 s), 2.1 Mbit/s measured on links 2 and 3,
  4.9 Mbit/s 300 ms later. Judged on the first measure, the stream went down to 1.3 Mbit/s for
  3 s. A report showing links cut now starts a window of its own, 300 ms
  (`ControllerSettings::cut_window_ms`), judged on its last measure: soon enough for a cut the
  others cannot carry (two links cut at 90 s: down 0.7 s after the cut). Waiting for a full
  window instead (500 ms more) lost 45 frames there in a lab run. Lab, one run: `real-ride` 2944
  kbit/s intact, 3 frames lost; `trace-drive` 2163, 188; `all-links-drop` 2510, 189.

CI run 45194700354 (3 runs, medians): our transport beats the baseline on every scenario.
`real-ride` 2950 kbit/s intact, 11 frames lost, longest gap 132 ms (baseline 2968, 52, 396 ms);
`trace-drive` 2118, 172, 924 ms (1051, 1754, 3333 ms); `all-links-drop` 2538, 186, 6.1 s (2434,
209, 6.9 s); `link-drop` 3056, restored link 50 % (3028, 13 %).

## Recording upload (M1b step 6)

The background class carries the full-quality recording to the server, on the capacity the live
class leaves, resumed across link outages (plan and Emilien's decisions:
`plans/m1b-step-6.md`).

**Everything is a setting, off by default.** One type per side, read from the environment by
the binaries only:

- `bonding_sender::freewheel::RecordingSettings`: `FREEWHEEL_RECORDING` (the recording's path;
  unset, no upload), then `FREEWHEEL_RECORDING_` + `CHUNK_BYTES` (1471, a full datagram),
  `SHARE_PERCENT` (of the room the live traffic leaves; 100), `LIVE_RATIO_PERCENT` (of the
  live bytes in flight on the link, while the live stream runs; 100), `QUEUE_TARGET_MS` (10),
  `BASE_RTT_WINDOW_S` (10), `WINDOW_GAIN_CHUNKS` (4), `IDLE_LIVE_BPS` (100000),
  `LOSS_TIMEOUT_MS` (1500),
  `END_IDLE_MS` (the recording has ended when it has not grown for that long; 2000), `DRAIN_S`
  (how long the upload keeps trying after that; 3600), `WINDOW_BYTES` (256 MiB beyond the first
  byte not delivered), `IN_FLIGHT_CHUNKS` (8192).
- `bonding_receiver::freewheel::RecordingSettings`: `FREEWHEEL_RECORDING_DIR` (where copies go;
  unset, background data is acknowledged and dropped), `FREEWHEEL_RECORDING_QUEUE_CHUNKS`
  (chunks waiting for the disk; 4096), `FREEWHEEL_RECORDING_ACK_RANGES` (ranges per ACK; 16).
- The lab: `RECORDING`, `RECORDING_BPS`, `RECORDING_DRAIN_S` (`labs/srtla-capture/README.md`,
  "Recording upload").

**Receiver.** With a recording (`Receiver::enable_recording`), each recording chunk goes to
`ReceiverIo::write_recording`; the server's (`RecordingWriter`) copies it into a bounded queue,
allocated once, and a thread of its own writes it at its offset in
`<FREEWHEEL_RECORDING_DIR>/<session id>.recording` (never truncated). A full queue refuses the
chunk at once: the live path never waits for the disk. A chunk taken is acknowledged by its
sequence: every ACK of the session, on any link, carries the latest runs of chunk sequences
taken (16 by default), so a lost ACK or a link dying right after costs no resend; a chunk
refused is not, and the sender sends it again. Padding and malformed payloads are counted and
dropped. Tests: `tests/freewheel_recording.rs` (out of order and repeated chunks rebuild the
recording, ACK ranges, a full disk, no recording), `tests/freewheel_recording_zero_allocations.rs`.

**Sender.** `RecordingUpload` (`bonding_sender::freewheel`) cuts the recording, a file that may
still be growing (`SenderIo::recording_bytes`, `read_recording`; `freewheel-sender` reads the
file with `pread`), into chunks of `CHUNK_BYTES`: chunk `i` is bytes `i × size` to
`(i + 1) × size`. It always sends the first chunk neither delivered nor in flight, so a lost
chunk goes before new ones, within `WINDOW_BYTES` of the first chunk not delivered (two bits per
chunk over the window: delivered, in flight). Each copy sent is remembered with its sequence and
link in a ring, in sending order, so an ACK's ranges find their copies by binary search. A copy
is delivered when an ACK's ranges hold its sequence; lost when an ACK of its link acknowledges a
later sequence without it, when its link stalls, closes or leaves, or after `LOSS_TIMEOUT_MS`.
A late delivery after a loss counts once. Everything lives in memory for the session: an
outage of every link only pauses it. While the file grows only whole chunks go; once it has not
grown for `END_IDLE_MS`, its tail goes too (sent again whole if the file grows after all), and
the upload gives up `DRAIN_S` later. The logs say when it ended, and when it is delivered (with
the chunks sent and lost) or abandoned.

**Scheduling.** Live first, always. Each link has a spare window, LEDBAT-like: slow start,
then up to `WINDOW_GAIN_CHUNKS` chunks per RTT while the link's queue (smoothed RTT minus its
lowest over one to two `BASE_RTT_WINDOW_S`) stays under `QUEUE_TARGET_MS`, down past it, halved
once per RTT past twice it. While the live traffic is idle (under `IDLE_LIVE_BPS` over the last
second, as during the drain), the target widens by the serialisation of four packets and twice
the RTT variation, so a slow or jittery link is not read as queuing. The window never exceeds
`SHARE_PERCENT` of the link's congestion window minus the live bytes in flight, and the live
traffic's decisions do not count the recording's bytes (they yield to it). While the live
stream runs, it also stays under `LIVE_RATIO_PERCENT` of the live bytes in flight on the link: a
link whose capacity drops at once delivers what it holds late, the live packets behind the
upload's included (CI 45278362883, `varying-bandwidth`, 8 → 1 Mbit/s: SRT resent what came late
and the encoder backed off for 3 s; on the simulated drop, live p99 80 ms without the upload,
347 ms uncapped, 160 ms at 100 %, Emilien's choice, `decisions.md` 2026-11-24). Each chunk goes to
the link whose spare window is the least full, so alike links carry alike shares and what the
live traffic measures of them stays alike. Probe runs carry chunks instead of padding (padding
only when no chunk is ready), and none starts while the upload has a backlog: the upload fills
every link's spare room itself, so what a link delivers is what it carries, and a run would
only stop the upload for the run and its settling, a third of every second (CI 45253171813:
`trace-drive` drained at 2.7 of its 3.96 Mbit/s). A link back from a stall is still probed at
once. With the upload on, packets that ACKs (of keepalives) pass for `LOSS_TIMEOUT_MS` without
taking out of flight are lost: the last ones sent as a link went quiet (a dead second of
`trace-drive`, shorter than a stall) would otherwise fill its window for good, and nothing would
be sent to find out (CI 45278362883: a link carried nothing for the whole drain). Tests:
`tests/recording_upload.rs` (scripted ACKs: losses, another link's ACK, an outage of every link
with each chunk delivered exactly once, timeouts, late deliveries, the window, the tail,
wrapping sequences), `tests/freewheel_recording.rs` (the real sender and receiver on the
simulated network: the copy is the recording byte for byte, through an outage of every link,
the live stream losing nothing, latency p99 with the upload on, a restored link's live share,
a backlog filling links that probing would interrupt, chunks lost as a link goes quiet, a
link whose capacity drops at once), and `tests/freewheel_zero_allocations.rs` with probing and the
upload on.

Off, both sides are step 5's to the bit: `tests/freewheel_recording_off.rs` digests every
datagram either side sends, with its time and link, over a simulated run with probing, an outage
of every link and SRT answers, and compares it with the digest of step 5's code.

**Results.** CI run 45352392251 (3 runs, medians), with a 6 Mbit/s recording (twice the live
ceiling) growing during every scenario: it arrives whole, byte for byte, in every run, after the
stream ends in 2 s (`all-links-drop`, `burst-loss`), 3 s (`link-drop`), 21 s (`trace-city`), 24 s
(`varying-bandwidth`), 73 s (`real-ride`) and 138 s (`trace-drive`, whose links carry 1–4 Mbit/s).
The live stream holds step 5's level (CI 45194700354): intact video `all-links-drop` 2538 kbit/s
(2538), `burst-loss` 3084 (3083), `link-drop` 3056 (3056, restored link 50 %), `real-ride` 2952
(2950), `trace-city` 3043 (3043), `trace-drive` 2182 (2118), `varying-bandwidth` 3046 (3046);
latency p99 +0 to +5 ms, within what the baseline itself moves between the two runs; frames lost
`real-ride` 18 (11), `trace-drive` 189 (172), the rest unchanged. What it took, in order: no probe
run while the upload has a backlog (CI 45253171813: `trace-drive` 95.6 %), a quiet link's lost
packets freed (CI 45278362883: 87.6 %), the live ratio cap (`varying-bandwidth` 2922 kbit/s
intact uncapped). Not built: resuming across a restart of either side (`roadmap.md`).

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
