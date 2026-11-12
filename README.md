# freewheel

Self-hosted mobile live streaming for long-distance bike travel. Written from scratch,
wire-compatible with [BELABOX](https://github.com/BELABOX), plus the telemetry it never had.

## Where we stand (2026-11-12)

Milestone **M1, link bonding in Rust with SRTLA compatibility**: step 5 of 6 is done. Nothing runs on a bike yet; everything below runs in emulation and CI.

- ✅ **M0 foundations**: monorepo, CI, deterministic cellular emulator (`link-emulator`), bench harness gating size, instructions and allocations.
- ✅ **SRTLA spec** written from packet captures of the upstream binaries (35 lab scenarios): [`docs/technical/protocol/srtla.md`](docs/technical/protocol/srtla.md).
- ✅ **Wire format** (`bonding-protocol`): every packet, encode + decode, fuzzed, zero allocation.
- ✅ **Receiver** (`bonding-receiver`): drop-in for `srtla_rec`. Upstream sender → our receiver passes the 40 characterisation tests.
- ✅ **Sender** (`bonding-sender`): drop-in for `srtla_send`, link selection as a pure state machine. Our sender → upstream receiver passes the same 40 tests; Rust ↔ Rust works on localhost.
- ✅ **End-to-end**: a real SRT stream through 2–4 links under `tc netem`, checked frame by frame. All four combinations (upstream ↔ upstream, both interop directions, Rust ↔ Rust) meet the same per-scenario limits in CI. Loss, jitter and bufferbloat cost no frame; a dying link costs at most a few.
- ⏭️ **Next**: bench against upstream and `irlserver/srtla_send`: throughput, CPU, memory, recovery time after a link drop.
- Later: our own transport (M1b), own server, bench hardware, encoder, agent, OS image, enclosure. See [`docs/technical/roadmap.md`](docs/technical/roadmap.md).

Compatibility mode reproduces upstream exactly, quirks included; fixes come with our own transport.

## Layout

| Path | What | Replaces |
|---|---|---|
| `crates/bonding-protocol` | Bonding wire formats, pure: our transport + SRTLA compatibility | `srtla` |
| `crates/bonding-receiver` | Bonding receiver for our server: SRTLA compatibility mode (pure state machine + one-thread UDP server) | `srtla_rec` |
| `crates/bonding-sender` | Bonding sender for the box: SRTLA compatibility mode (pure state machine + one-thread UDP server) | `srtla_send` |
| `crates/net-sys` | The few C calls std lacks (`poll`, `setsockopt`, `signal`), behind safe wrappers | — |
| `crates/bitrate-control` | Adaptive bitrate controller, pure | `belacoder` |
| `crates/alloc-counter` | Counts allocations in tests, to prove hot paths allocate nothing | — |
| `crates/benchmarks` | Hot-path benchmarks, one per file | — |
| `crates/packet-capture` | Reads pcap captures; reports what upstream SRTLA does on the wire | — |
| `crates/link-emulator` | Deterministic cellular emulator: traces, modem buffer, bursty loss, outages, handovers, address changes, reboots, uplink + downlink, radio power, data meter, synthetic traces, road scenarios | — |
| `apps/agent` | On-board agent (Bun + Effect, one ARM64 executable): network, modems, live supervisor, telemetry, tunnel | `belaUI` |
| `apps/server` | Own server (Bun + Effect): telemetry ingest, outage scenes, bets, replays | BELABOX cloud |
| `apps/web` | TanStack Router + Effect: the box's control panel and the server dashboard | belaUI front |
| `packages/domain` | Pure domain shared by every app (DDD, Effect Schema): telemetry outbox, positions… | — |
| `packages/typescript-config` | Shared TypeScript config | — |
| `scripts/bench` | Measures and gates performance | — |
| `scripts/fetch-traces` | Downloads public cellular traces | — |
| `labs/srtla-capture` | Docker lab running upstream SRTLA to capture its behaviour | — |
| `hardware/` | Enclosure and custom boards | — |
| `docs/` | Architecture, testing strategy, decisions | — |

## Principles

- **Rust where bytes flow, Bun + Effect where we orchestrate.** Bonding, encoder and bitrate control are Rust; agent, server and UI are TypeScript.
- **No dependency we don't need.** Zero third-party crates in Rust; every package is justified in `docs/technical/decisions.md`.
- **Pure core, thin adapters.** Protocol, bitrate and telemetry logic are pure and exhaustively tested.
- **Hardware is emulated in CI.** Links, camera, modems and GPS all have deterministic stand-ins; see `docs/technical/testing.md`.
- **Wire-compatible with upstream, not derived from it.** Upstream code is a reference for behaviour only; we write specs in `docs/technical/protocol/` and implement from them. Interop tests run the original binaries at pinned commits.

## Develop

```sh
bun install
bun run check        # Biome
bun run check-types  # TypeScript
bun run test         # bun test + cargo test
cargo clippy --workspace --all-targets -- -D warnings
bun run agent:build  # one ARM64 executable for the box
bun run traces:fetch # public cellular traces into .cache/ (for public-trace tests)
bun run bench        # size, instructions per packet, agent size, against the baseline
```

## Documentation

- **Users**: [`docs/guide/`](docs/guide/README.md), plain language, no technical background needed.
- **Builders**: [`docs/technical/`](docs/technical/README.md): architecture, code style, emulator, benchmarks, roadmap, decisions.

## License

MIT. Upstream BELABOX is copyleft (`srtla`: AGPL-3.0, `belacoder` and `belaUI`: GPL-3.0): never copy or translate its code, only observe its behaviour.
