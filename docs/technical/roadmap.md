# Roadmap

Target: road-ready by end of 2027. Each milestone ends with a "done when" that a test or a measurement proves.

## M0 — Foundations ✅
Monorepo, CI, `link-emulator`, first pure crates, telemetry outbox.

## M1 — Link bonding in Rust: SRTLA compatibility (no hardware needed) ✅
0. **Prerequisites** (from `docs/technical/research/00-synthesis.md`): ✅ `link-emulator` v3 (traces, modem buffer, burst loss, outages, correlated links, reverse path, handovers, address changes, modem reboots, radio power, data meter, synthetic traces, scenarios); ✅ public traces wired in (fetch + synthetic); ✅ `bench/` harness with size, instruction and allocation gates.
1. ✅ **Spec**: write `docs/technical/protocol/srtla.md` from observed behaviour: packet captures of the upstream binaries in a Linux VM, plus reading upstream for intent. Registration (REG1/2/3, errors), keepalives, ACK/NAK handling, link selection and congestion window, timeouts, group and connection limits. Done: `docs/technical/protocol/srtla.md`, lab in `labs/srtla-capture/`, analyser and wire tests in `crates/packet-capture`.
2. ✅ **Wire format**: `bonding-protocol` covers every packet, encode + decode, property-tested (`decode(encode(x)) == x`, no panic on any input, fuzzed). Done: `Packet::decode` / `encode`, zero allocation, every datagram of the 35 lab scenarios round-trips (`srtla.md`, "How `bonding-protocol` decodes").
3. ✅ **Receiver first** (`bonding-receiver`): it runs on our server, and upstream calls theirs unsupported. Interop: upstream sender → our receiver. Done: pure state machine + one-thread `poll` server (`net-sys`), a drop-in for `srtla_rec`; the 35 lab scenarios run with it (`RECEIVER=freewheel`) and pass the same 40 characterisation tests as upstream (`srtla.md`, "How `bonding-receiver` behaves"); CI job `interop`.
4. ✅ **Sender** (`bonding-sender`): link selection as a pure, tested state machine. Interop: our sender → upstream receiver. Done: pure state machine + one-thread `poll` server, a drop-in for `srtla_send` (`SIGHUP` reload included, via `net-sys`); the 35 lab scenarios run with it (`SENDER=freewheel`) and pass the same 40 characterisation tests (`srtla.md`, "How `bonding-sender` behaves"); Rust ↔ Rust checked on localhost; CI job `interop-sender`.
5. ✅ **End-to-end**: 2–4 links in network namespaces with `tc netem` (loss, jitter, outages, one link dying), a real SRT stream through it (`srt-live-transmit` both ends), checked frame by frame. Done: the encoder keeps what it sent, both sides are hashed per compressed frame (`framemd5`), `FrameCheck` in `packet-capture` counts intact, damaged, late-start and cut frames and the longest gap; `tests/end_to_end.rs` sets a limit per scenario (24 of the 35) and all four combinations meet it (upstream ↔ upstream, both interop directions, Rust ↔ Rust); CI job `rust-to-rust`. The encoder starts once SRT is connected, so the handshake time no longer costs a GOP and skews the start of the run. Measured over two campaigns of all four (2026-11-12): loss, bursts, jitter, bufferbloat and asymmetric links are fully hidden (0–6 frames, SRT dropping a late packet on a loaded host, upstream ↔ upstream `steady` included); a dying link costs 0–11 frames, upstream and ours alike; a 6 s total outage leaves a 4.8–7.1 s gap, then the stream recovers. Not yet measured beyond noise, for step 6: the gap under `congested` varied 0.6–2.0 s and one `two-senders` run lost 263 frames, neither reproduced in the second campaign.
6. ✅ **Bench** against upstream and `irlserver/srtla_send` (Rust, MIT): throughput, CPU, memory, recovery time after a link drop. Done: `labs/srtla-capture/bench.sh` + `bench-srtla`, CI job `srtla-bench` with a gate against upstream; results and reading in `bench/README.md` ("SRTLA bonding"). On par: same delivery and recovery, CPU within 11 %, memory within 0.25 MB. irlserver pays 2× the sender CPU and 5× the memory for a much faster return of a restored link (47 % of the traffic 15–25 s later, against 12 %), the first thing M1b must beat.

Done when: both interop directions and Rust ↔ Rust pass the e2e suite in CI, and the bench is at least on par with upstream. This is the baseline for M1b.

## M1b — Our own transport ← now
Design in `docs/technical/protocol/freewheel.md`: three traffic classes (live, reliable, background recording upload), per-link estimation, arrival-time scheduler, adaptive FEC, deadline-aware retransmission, cross-layer bitrate, session ids, Noise encryption. Prototype candidates, benchmark them against the SRTLA baseline on the same emulated links and recorded traces. The server speaks both protocols.

1. ✅ **Bench for M1b**: the measures of "How we decide", per scenario, on the same lab as M1. Done: `frame-times` dates every video frame at the encoder's output and at the SRT listener's (`LabRun::latency`: p50 / p99 over the intact frames), delivered bitrate per run, `replay_trace` replays recorded LTE uplinks (`trace-city`, `trace-drive`), `labs/srtla-capture/bench-transport.sh` + `bench-transport` (table, JSON, `--gate` for a candidate against the baseline), CI job `transport-bench`. Baseline in `bench/README.md` ("Transport"). The recording measure waits for the background class (step 6).
2. ✅ **Wire format v0** in `bonding-protocol`: session ids, the three classes, acknowledgements carrying per-link timing; same proofs as M1 (round trip to the byte, fuzzing, no allocation). Done: `bonding_protocol::freewheel` (layout in `docs/technical/protocol/freewheel.md`, "Wire format v0"): 12-byte header with magic `0xF0`, session and link ids; data, ACK, keepalive, join, accept, close; tests `freewheel_round_trip`, `freewheel_decode`, `freewheel_fuzz`; benchmark `freewheel_decode`.
3. ✅ **First candidate**: per-link estimation + scheduling by expected arrival. Must beat irlserver's return to a restored link (judged as an even split back: irlserver overshoots) and the baseline on `trace-drive`. Done: `freewheel-sender` / `freewheel-receiver`, lab pair `transport/transport` (design and results in `docs/technical/protocol/freewheel.md`, "First candidate"), `--gate transport-transport` on in CI. CI, 3 runs (run 44151066706): `trace-drive` 497 frames lost against 1743 (irlserver 1960), 2903 kbit/s against 1832, p50 +18 ms; a restored link carries 48–50 % of the traffic 15–25 s after (an even split; gated at 45 %), against 13 % for the baseline; irlserver overshoots (42–77 %). Queuing beyond the windows is bounded at 500 ms (SRT reads a full modem queue as clock drift). Not won: seconds where the links carry less than the stream (`trace-drive` 17–24 s, `real-ride` 90–93 s), which wait for the cross-layer bitrate (step 5). 2026-11-18: the gate's slack on frames lost includes the run-to-run spread of the scenario (`real-ride`: 49–63 frames over four CI runs, ours and the baseline alike), stable scenarios keep +5 % or +3 frames. Late start under `random-loss` (6–20 s, CI only): one lost REG2 answer, then REG1 retried from the same address and refused until the orphan group expires; compatibility sender now retries on the next link, and the scenario's loss starts once the stream runs (`srtla.md`, "Fragile start").
4. ~~**Deadline-aware retransmission**~~ (not kept, 2026-11-22: no gain on the bench, `decisions.md`; branch `m1b-step-4`), ~~**adaptive FEC**~~ (not built, 2026-11-22: no frame is lost to isolated losses, all to missing capacity and the catch-up after; `LossCauses`, `freewheel.md`, "Why frames are lost"), then **media-aware priorities**: one at a time, each kept only if the bench says so.
5. **Cross-layer bitrate** ← now: the transport tells `bitrate-control` the aggregate capacity, and the lab's encoder lowers its bitrate when the links cannot carry it. 2026-11-23: the gate judges the intact video (`decisions.md`); capacity probing (≤ 10 % of the capacity, only below the encoder's ceiling) brings the encoder back within 1–3 s of an outage. CI run 45135774902: `trace-drive` 1906 kbit/s intact against 1093, but `all-links-drop` (2190 against 2434) and `real-ride` (2801 against 2974) still below the baseline (`freewheel.md`, "Cross-layer bitrate and capacity probing"). Then: `lab-encoder` follows a raise at once (capped CRF instead of x264's ABR, ~10 s), after a cut of some links the controller holds a stream that fits what the others carry, climbs at once when links come back, and an outage of every link holds the bitrate. Lab, one run: `all-links-drop` 2683 kbit/s intact, 132 frames lost (baseline 2434, 209), `real-ride` 2890–2934, 2–8 frames lost (baseline 2974, 45), `trace-drive` 2114, 172 frames lost (baseline 1093, 1717).
6. **Background class**: the full-quality recording uploaded on spare capacity, resumed after outages; the bench measures the share delivered.
7. **Noise** handshake + AEAD from an audited crate (decision in `decisions.md`).
8. **Multipath QUIC** with datagrams on the same bench, before freezing the choice.

Done when: on the benchmark suite it beats the baseline on lost frames and delivered bitrate, with equal or lower latency, delivers 100 % of the recording after outages, and starts on time with any single control packet lost (join, accept, SRT handshake answer), on any link: no lab scenario may start late.

## M2 — Own server
Receiver + SRT listener, relay to YouTube/Twitch, telemetry ingest and storage, WireGuard endpoint. Deployed on a VPS with infrastructure as code.

Done when: a test stream from a laptop goes live on YouTube through our server, and survives a scripted link outage.

## M3 — Bench hardware
ROCK 5T + modem + Osmo Action on the desk, running the stock BELABOX image as the reference.
- Measure real watts (USB-C tester), temperatures, camera UVC formats (`v4l2-ctl --list-formats-ext`).
- Record real traces (link stats during a ride, GPS, power) and replay them in `link-emulator` and the agent tests.
- Swap stock `srtla_send` for `bonding-sender`.

Done when: a 10-hour bench run streams through our bonding with no manual action.

## M4 — Encoder in Rust
`bitrate-control` (pure bitrate controller, simulated against recorded traces), GStreamer pipeline UVC MJPEG → MPP H.265 → SRT. Software encoder in CI, MPP on the board.

Done when: it replaces `belacoder` on the bench and handles the recorded traces better or equal.

## M5 — Agent (Bun + Effect) and web UI replace belaUI
- NetworkManager and ModemManager over D-Bus: links, Wi-Fi, hotspot, SIM, APN, roaming.
- Live supervisor: start, stop, restart encoder and bonding, readable errors.
- Telemetry with store-and-forward: GPS, status, temperatures, power.
- Tunnel to our server, remote control, mobile web UI.
- Later: ingest a phone as a second camera.

Done when: the stock image is gone from the bench and every feature above has fake-hardware tests.

## M6 — Our own system, pro and safe
Minimal reproducible OS image for the ROCK 5T, signed A/B updates pulled from our server with automatic rollback, read-only root, SSH by key only and only through the tunnel, firewall, secrets out of the image.

Done when: an update can be pushed from the server, a broken update rolls back by itself, and a fresh board is provisioned from scratch in one command.

## M7 — Enclosure and field tests
3D-printed enclosure (waterproof, ventilated, light, several iterations). Tests: Breton rain, freezer (−20 °C), car in full sun, 10 hours in a row, 3 days without mains power.

Done when: the 3-day test passes with no lost GPS point and no lost recording.

## M8 — Custom hardware
Power and sensor board, then possibly an RK3588 carrier board, with a quality manufacturer in China (JLCPCB / PCBWay for prototypes, an ODM once stable).

## M9 — Native mobile app
Our own iOS (Swift) and Android (Kotlin) app, native for performance and battery: phone as a second camera, a backup streamer when the box is down, and the box's remote control. The Rust transport crates are embedded through a thin C ABI, so the phone speaks exactly the same protocol as the box.

Done when: the phone streams to our server over our protocol, bonding its cellular and Wi-Fi, and the box can be controlled from it.

