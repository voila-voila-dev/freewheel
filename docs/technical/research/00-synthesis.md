# Research synthesis (2026-11-10)

Five parallel studies: `01-state-of-the-art.md`, `02-transport-mechanisms.md`,
`03-media-aware-priorities.md`, `04-security-operations.md`, `05-embedded-performance.md`.

## What makes freewheel the best (no one does these together)

1. **Live + background recording backfill in one session.** IRL apps only record locally; LiveU's Store & Forward is a separate mode. We upload the full-quality recording with spare capacity, resume across outages, and fill the VOD holes ("what you missed").
2. **User-owned policy**: data cost per SIM (€/GB), battery, latency, quality, privacy. Presets (Cinéma, Talk show, Action, Économie de data, Batterie, Silence radio) + an "Événement" boost button.
3. **Media- and deadline-aware engine**: earliest-arrival scheduler with per-class duplication (audio, keyframes), temporal layers dropped instead of random packets, recovery from the last frame the server confirmed (MPP long-term references) instead of a full keyframe.
4. **Cross-link FEC**: sliding-window RLC (RFC 8681) with repair on a different link than its data, so a dying link is recoverable. Patent-safe path (no RaptorQ / RLNC).
5. **Sessions that survive ~15 min outages**, links joining instantly, shadow warm-up of returning links, server-side outage scenes.
6. **Secure by default, censorship-resilient**: Noise IK handshake, no open relay, pluggable outer layer (UDP 443 → TLS/TCP 443 → WebSocket via CDN).
7. **A public benchmark on recorded cellular traces**: none exists today. Also our marketing.

## Key facts learned

- Cellular uplinks suffer from delay (modem buffers up to ~4 s) more than loss: the pacer keeps queues in our process.
- YouTube/Twitch add seconds anyway: a 0.8–2 s transport budget allows ARQ as backbone, FEC as accelerator; a ~300 ms profile leans on FEC.
- Rockchip MPP exposes temporal layers, 16 LTR frames, intra refresh, ROI, per-frame SSE/QP; GStreamer `mpph265enc` exposes none → own MPP FFI for the `encoder` crate.
- RK3588 A76 and A55 report AES/PMULL: crypto costs a few % of one A55 at most.
- The 109 MB agent is the Bun runtime; `--trim`, `--trim-locales`, `--bytecode` → ~30–45 MB expected; ship `zstd --patch-from` deltas.
- GitHub `ubuntu-24.04-arm` runners available for private repos: native ARM64 CI, no QEMU for performance.
- Baselines to beat: upstream SRTLA (AGPL-3.0) **and** `irlserver/srtla_send` (Rust, MIT, actively maintained).
- MP-QUIC (draft-21, approved) and MoQ (draft-19) are not a base: borrow MoQ's media model (groups = GOPs, priorities), not its wire format.

## Proposed decisions (to confirm)

| Area | Proposal |
|---|---|
| Async I/O | std + poll, `sendmmsg`/`recvmmsg`, UDP GSO/GRO, `SO_TXTIME` pacing, one small FFI crate for syscalls |
| Congestion control | SCReAM-style per link + BBR-style delivery-rate sampling |
| FEC | Sliding-window RLC + Tetrys-style ack window, burst-loss-adaptive rate |
| Receiver | Per-link sequence numbers, deadline-aware ARQ, fixed playout offset |
| Encoder | Own MPP FFI; dual encoding (adaptive live + fixed high-quality recording) |
| Audio | Opus on two links, in-band FEC (+ DRED to evaluate) |
| Crypto | `snow` (audited) for Noise IK; AEAD chosen by benchmark (ChaCha20-Poly1305 vs AES-GCM) on the board |
| Server | Production switcher (decode + re-encode), warm standby on a second VPS |
| Fuzzing | `fuzz/` outside the workspace with `libfuzzer-sys`; property tests and Kani in-tree |
| Perf gates | 0 alloc/packet; instructions/packet +2 %; binary size +2 %; agent size +3 %; memory +10 % |
| Budget | 10 W at 6 Mbps / 2 links; sender ≤ 5 % of one A55, ≤ 16 MB, ≤ 0.2 W; agent ≤ 60 MB RAM, ≤ 45 MB flash |
| Privacy | `PublicPosition` type in `packages/domain`: delayed, blurred, hidden when stationary |

## Blocking prerequisite

`link-emulator` only models random loss and jitter. Before any comparison it needs: trace-driven capacity (Mahimahi format, public LTE traces), a finite modem buffer, burst loss, scripted outages, correlation between links on the same tower.

## Next steps

1. `link-emulator` v2 (above) + `bench/` harness with size, instruction and allocation gates.
2. SRTLA spec + compatibility mode (M1), benchmarked against upstream and `irlserver/srtla_send`.
3. Prototype the engine (M1b) on the emulator, experiments E1–E10 from `02-transport-mechanisms.md`.
