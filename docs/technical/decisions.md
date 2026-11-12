# Decisions

One line per decision, newest last. A third-party dependency needs an entry here.

- 2026-11-10 — Monorepo like tries.care: Rust workspace (`crates/`), Bun workspaces (`apps/`, `packages/`), Biome, TypeScript 7, catalogs for versions.
- 2026-11-10 — MIT, written from scratch. Upstream (`srtla` AGPL-3.0, `belacoder` and `belaUI` GPL-3.0) is a behavioural reference only: no copied or translated code. Protocol facts (packet types, lengths, timeouts) are documented in `docs/technical/protocol/` and implemented from there.
- 2026-11-10 — Rust crates: no third-party crates by default. `unsafe` forbidden except in dedicated FFI crates.
- 2026-11-10 — Rust where bytes flow (bonding, encoder, bitrate), Bun + Effect where we orchestrate (agent, server). The agent ships as one executable (`bun build --compile --target=bun-linux-arm64`), so the OS image has no JS runtime to install. Supersedes the all-Rust + Dioxus option.
- 2026-11-10 — `packages/domain` holds the pure model shared by agent, server and web, in the tries.care style (bounded contexts: aggregates, value-objects, errors, use-cases).
- 2026-11-10 — Web UI: TanStack Router + React + Effect, one app for the box's panel and the server dashboard. React, TanStack and Vite are accepted dependencies for the UI only.
- 2026-11-10 — Rust ↔ agent contract: local Unix socket, JSON messages, schemas defined in `packages/domain`.
- 2026-11-10 — Talk to NetworkManager / ModemManager over D-Bus, not by parsing `nmcli` / `mmcli` output.

- 2026-11-10 — Target board: Radxa ROCK 5T only (RK3588). No Jetson or other boards.
- 2026-11-10 — Linux dev and e2e tests run in an OrbStack VM on macOS, and on Ubuntu in CI.
- 2026-11-10 — Crate names say what they do: `bonding-protocol`, `bitrate-control`, `link-emulator`; later `bonding-sender`, `bonding-receiver`, `encoder`.
- 2026-11-10 — Own transport protocol (`docs/technical/protocol/freewheel.md`), plus SRTLA compatibility as a mode and as the benchmark baseline (also opens the IRL streaming market: phone apps and other boxes speaking SRTLA).
- 2026-11-10 — Cryptography only from an audited crate; never implemented in-house. Accepted exception to the no-dependency rule.
- 2026-11-10 — SRTLA-specific code lives in one module (`bonding-protocol/src/srtla.rs`); everything else uses generic bonding names.
- 2026-11-10 — Own native mobile apps (Swift, Kotlin), embedding the Rust transport crates through a C ABI. No cross-platform UI framework.

- 2026-11-10 — AEAD (ChaCha20-Poly1305 vs AES-GCM) decided by a benchmark on the ROCK 5T in M3.
- 2026-11-10 — `link-emulator` replays public cellular traces (Mahimahi format) until our own traces exist (M3).
- 2026-11-10 — CDN fallback (WebSocket through a CDN) accepted as the last resort in censoring countries.
- 2026-11-10 — A public benchmark on real cellular traces is a first-class deliverable (and our selling point).
- 2026-11-10 — Encoder drives Rockchip MPP through our own FFI, not GStreamer `mpph265enc`.
- 2026-11-10 — Dual encoding: adaptive live + fixed high-quality recording.
- 2026-11-10 — Server is a production switcher (decode + re-encode), not a relay.
- 2026-11-10 — Audio sources: the camera's microphone (UAC over USB) and an external receiver such as DJI Mic plugged into the box (USB audio); selectable, both recorded.
- 2026-11-10 — Borrow ideas from MP-QUIC and MoQ, not their wire formats.

- 2026-11-10 — Recording backfill is optional and policy-driven (off, outages only, highlights, everything; per link type: cellular vs Wi-Fi). The recording is cut into numbered, checksummed segments; the server tracks which ones it holds and only missing segments are sent. Nothing is rebuilt from the live stream (different encode).
- 2026-11-10 — Outage behaviour on the server is a setting: outage scene (animations, bets, highlights), raw (a few seconds of the real cut, no frozen frame, then the reconnection moment), or mixed (raw for a few seconds, then the scene). No frozen "connection lost" frame: it reads as alarming. "What you missed" replays are optional and never shift the live.
- 2026-11-10 — Mobile app audio: phone microphone or an external microphone (e.g. DJI Mic receiver on USB-C, or Bluetooth).

- 2026-11-10 — Safe deletion of recordings on the box: a segment can be deleted only after the server confirms it is stored (written to disk, checksum verified). Manual or automatic deletion; upload progress visible from the phone.
- 2026-11-10 — Default policy: cellular for live; full-quality upload on Wi-Fi, or on cellular when the plan allows.
- 2026-11-10 — Data accounting per SIM, keyed by the SIM card (ICCID): bytes up/down and throughput, full history kept, user-set plan cap (e.g. 25 GB), alerts, and a user-set renewal date (operators do not expose it).
- 2026-11-10 — Mobile app (iOS first, Android too): two screens, box control and live control (HUD, scenes, highlights, "what you missed"). Chat on the phone can be switched off to save data. Every feature can be turned on or off; the interface stays simple. Microphone picker like iOS.
- 2026-11-10 — Synthetic traces generated from statistics of public traces, so the benchmark does not depend on third-party files.

- 2026-11-10 — Code style: one thing per file, explicit names with units, layered TypeScript as in tries.care (`docs/technical/code-style.md`).
- 2026-11-10 — Two kinds of documentation: `docs/technical/` for builders, `docs/guide/` in plain language for streamers who are not technical. English first, French next.
- 2026-11-10 — `alloc-counter` is the first crate allowed `unsafe` (a global allocator), never linked into the box.
- 2026-11-10 — One benchmark baseline per platform; the CI gate runs on native `ubuntu-24.04-arm`.
- 2026-11-10 — Upstream behaviour is studied in a Docker lab (`labs/srtla-capture/`, Debian, upstream built at the pinned commit) rather than a full VM: same ARM64 Linux under OrbStack, reproducible in CI. Inside, network namespaces (box, net, server) with `tc netem` per link and direction, and 35 scenarios, one file each, covering baseline, link quality, outages, real conditions, addressing, lifecycle, limits and abuse. Each has a characterisation test; the same scenarios will judge our receiver and sender. Captures stay in `.cache/`; only a 120-packet fixture is committed.
- 2026-11-10 — The SRTLA compatibility mode reproduces upstream as is, quirks included (4 s dead-link timeout, slow ramp after recovery, unflushed ACK batches). Improvements belong to our own transport (M1b), so interop stays exact.
- 2026-11-11 — Fuzzing without a third-party fuzzer: seeded tests (exhaustive short inputs, every type at boundary lengths, random mutations of valid packets) run in `cargo test`, deterministic and on stable. Coverage-guided `cargo-fuzz` (libFuzzer, nightly) can be added later if a decoder grows complex enough to need it.
- 2026-11-11 — Event loop for the bonding receiver and sender: one thread, std blocking UDP sockets, and `poll(2)` through our own `net-sys` crate, the second crate allowed `unsafe` (research 05). `recvmmsg`/`sendmmsg` or io_uring later, measured, behind the same wrapper. Closes the open "async runtime" question.
- 2026-11-11 — Each bonding crate keeps its SRTLA-specific code in one `srtla` module (`bonding-protocol`, `bonding-receiver`, later `bonding-sender`); refines the single-module entry above.
- 2026-11-11 — Interop is judged by the lab: `RECEIVER=freewheel` runs the 35 scenarios with our receiver in place of `srtla_rec` (same command line), and the same characterisation tests must pass on those captures. Same plan for the sender.
- 2026-11-11 — `net-sys` also wraps `setsockopt(SO_SNDBUF)` (upstream's 8 MB send buffer per link) and `signal(SIGHUP)` (belaUI reloads the sender's link list with it; std has neither). Still the only bonding crate with `unsafe`; one `SAFETY:` comment per call. The sender's link sockets are non-blocking, as upstream's: a full buffer is a failed send, which disables the link.

## Open

- Which company owns freewheel (IP, sales): Hopla LLC (US) or VOILA DEVELOPMENT (France).

- SRT: thin own FFI to `libsrt` vs a pure-Rust implementation.
- GStreamer from Rust: own FFI vs `gstreamer-rs` (would need an entry here).
- D-Bus for NetworkManager / ModemManager from Bun: own minimal client vs a library.
- OS image and updates: base system, signed A/B updates pushed from the server.
