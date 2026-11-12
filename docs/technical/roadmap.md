# Roadmap

Target: road-ready by end of 2027. Each milestone ends with a "done when" that a test or a measurement proves.

## M0 — Foundations ✅
Monorepo, CI, `link-emulator`, first pure crates, telemetry outbox.

## M1 — Link bonding in Rust: SRTLA compatibility (no hardware needed) ← now
0. **Prerequisites** (from `docs/technical/research/00-synthesis.md`): ✅ `link-emulator` v3 (traces, modem buffer, burst loss, outages, correlated links, reverse path, handovers, address changes, modem reboots, radio power, data meter, synthetic traces, scenarios); ✅ public traces wired in (fetch + synthetic); ✅ `bench/` harness with size, instruction and allocation gates.
1. ✅ **Spec**: write `docs/technical/protocol/srtla.md` from observed behaviour: packet captures of the upstream binaries in a Linux VM, plus reading upstream for intent. Registration (REG1/2/3, errors), keepalives, ACK/NAK handling, link selection and congestion window, timeouts, group and connection limits. Done: `docs/technical/protocol/srtla.md`, lab in `labs/srtla-capture/`, analyser and wire tests in `crates/packet-capture`.
2. ✅ **Wire format**: `bonding-protocol` covers every packet, encode + decode, property-tested (`decode(encode(x)) == x`, no panic on any input, fuzzed). Done: `Packet::decode` / `encode`, zero allocation, every datagram of the 35 lab scenarios round-trips (`srtla.md`, "How `bonding-protocol` decodes").
3. ✅ **Receiver first** (`bonding-receiver`): it runs on our server, and upstream calls theirs unsupported. Interop: upstream sender → our receiver. Done: pure state machine + one-thread `poll` server (`net-sys`), a drop-in for `srtla_rec`; the 35 lab scenarios run with it (`RECEIVER=freewheel`) and pass the same 40 characterisation tests as upstream (`srtla.md`, "How `bonding-receiver` behaves"); CI job `interop`.
4. ✅ **Sender** (`bonding-sender`): link selection as a pure, tested state machine. Interop: our sender → upstream receiver. Done: pure state machine + one-thread `poll` server, a drop-in for `srtla_send` (`SIGHUP` reload included, via `net-sys`); the 35 lab scenarios run with it (`SENDER=freewheel`) and pass the same 40 characterisation tests (`srtla.md`, "How `bonding-sender` behaves"); Rust ↔ Rust checked on localhost; CI job `interop-sender`.
5. ✅ **End-to-end**: 2–4 links in network namespaces with `tc netem` (loss, jitter, outages, one link dying), a real SRT stream through it (`srt-live-transmit` both ends), checked frame by frame. Done: the encoder keeps what it sent, both sides are hashed per compressed frame (`framemd5`), `FrameCheck` in `packet-capture` counts intact, damaged, late-start and cut frames and the longest gap; `tests/end_to_end.rs` sets a limit per scenario (24 of the 35) and all four combinations meet it (upstream ↔ upstream, both interop directions, Rust ↔ Rust); CI job `rust-to-rust`. Measured 2026-11-12: loss, bursts, jitter, bufferbloat and asymmetric links cost no frame; a dying link costs 0–7 frames with ours (up to 11 upstream); a 6 s total outage leaves a 4.8–7.1 s gap, then the stream recovers. To look at in step 6: `congested` gap 2.0 s with our receiver against 0.6 s upstream; `two-senders`, our sender → upstream receiver, lost 263 frames once (not in the limits yet).
6. **Bench** against upstream and `irlserver/srtla_send` (Rust, MIT): throughput, CPU, memory, recovery time after a link drop.

Done when: both interop directions and Rust ↔ Rust pass the e2e suite in CI, and the bench is at least on par with upstream. This is the baseline for M1b.

## M1b — Our own transport
Design in `docs/technical/protocol/freewheel.md`: three traffic classes (live, reliable, background recording upload), per-link estimation, arrival-time scheduler, adaptive FEC, deadline-aware retransmission, cross-layer bitrate, session ids, Noise encryption. Prototype candidates, benchmark them against the SRTLA baseline on the same emulated links and recorded traces. The server speaks both protocols.

Done when: on the benchmark suite it beats the baseline on lost frames and delivered bitrate, with equal or lower latency, and delivers 100 % of the recording after outages.

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

