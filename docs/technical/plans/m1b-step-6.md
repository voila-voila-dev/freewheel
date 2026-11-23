# Plan: M1b step 6, the background class (recording upload)

Handoff for the agent taking step 6 of `roadmap.md` (M1b, "Background class"). Written
2026-11-23 at the end of step 5. Read this whole page, then `roadmap.md` (M1b),
`protocol/freewheel.md` (all of it, step 5's section last) and the 2026-11-2x entries of
`decisions.md` before touching code.

## Goal

The full-quality recording reaches the server, uploaded on the links' spare capacity, resumed
after outages, without hurting the live stream. From the roadmap's "done when" for M1b: the
transport "delivers 100 % of the recording after outages". The bench's "Recording delivered"
column, empty today (`bench_transport.rs`, "not measured yet"), gets a value.

## State at handoff

- Branch `m1b-step-5` (commit `a437b91`), not merged yet. CI run 45194700354 was still running
  at handoff: its `transport-bench` job decides whether step 5 is done (`real-ride` must reach
  2911 kbit/s intact, the baseline's 2970 less 2 %; lab, one run: 2944). In the same run,
  `srtla-lab` (`group_limit`) and `interop-sender` (`srt_server_restart`) failed on
  characterisation tests of the upstream server that passed the run before on the same code
  (except for a bitrate-control change they do not use): probably flaky. Rerun them
  (`gh run rerun 45194700354 --failed`) before concluding anything.
- If `transport-bench` passed: mark step 5 ✅ in `roadmap.md`, open the PR `m1b-step-5` → `main`,
  and start step 6 from it (`m1b-step-6`). If it failed: finish step 5 first.
- What step 6 builds on, already there:
  - **Wire format**: the `Background` class (`0x03`) exists (`bonding-protocol/src/freewheel/
    traffic_class.rs`); data payloads are opaque; ACKs carry the session's received sequence
    ranges, on all links and classes (`ack.rs`, `sequence_range.rs`).
  - **Sender**: capacity probing sends background padding on the links' spare room, within
    a budget of 10 % of the estimated capacity (`bonding-sender/src/freewheel/probe.rs`,
    `sender.rs` around `TrafficClass::Background`). "Padding is the stand-in for the recording
    upload": the same class, the same spare capacity.
  - **Receiver**: background data is counted and acknowledged, then dropped
    (`bonding-receiver/src/freewheel/receiver.rs`, "Background data is counted…").

## What to decide with Emilien first

Ask these questions before building; each one is his call, and the answer goes to `decisions.md`.
Give a recommendation with each.

1. **What the lab records.** Recommended: the sender reads a file that grows during the
   run (the "recording"), written by the lab at a constant rate (e.g. 6 Mbit/s, 2× the live
   ceiling, as a camera's full-quality file would be), random bytes, so the receiver's copy can
   be compared byte for byte. The real box later points it at the encoder's segments.
2. **How long the upload may take after the stream ends** (the drain time the bench allows
   before judging "100 %"). Recommended: as long as the scenario takes, the links kept up after
   the stream ends, and the bench reports the time to 100 %.
3. **How much the background may take.** Recommended: only what the live class leaves, never
   queuing the live traffic. The gate's latency p99 and intact video must not regress against
   step 5's numbers.
4. **Scope of "resumed".** Recommended for step 6: across link outages, with the sender and the
   receiver running all along. A sender or receiver restart (the box reboots) needs the
   receiver to tell what it has; write it down as a later step, not now.

## Proposed design (to confirm on the bench)

- **Addressing.** A background payload starts with a small header of its own: the offset of
  its bytes in the recording (64 bits), then the bytes. Document it in `freewheel.md`, "Wire
  format v0", with the same strict decoding and the same proofs (round trips, exact layouts,
  fuzzing, no allocation). An offset makes a resend idempotent: the receiver writes the bytes
  where they belong, whichever copy arrives.
- **Sender.** A recording reader ahead of the links: the next offset not yet sent, and the
  chunks in flight (sequence → offset, length). ACK ranges mark chunks delivered. A chunk is
  lost when later sequences are acknowledged on its link and it is not, or after a timeout;
  then it goes back to the head of the queue. Its state lives in memory for the whole session,
  so an outage only pauses it.
- **Scheduling.** Live first, always. The background fills what the congestion window leaves
  after live, on links the scheduler does not find full, and the probe runs carry recording
  bytes instead of padding (probing keeps its rules and budget; padding only when there is no
  recording to send). Start conservative (the probe runs plus a small share of the spare room),
  then open up while the bench shows no harm to live.
- **Receiver.** Writes each chunk at its offset in a file per session
  (`FREEWHEEL_RECORDING_DIR`), never on the live path's critical section. A write that fails
  must not touch the live stream.
- **Bench.** The lab writes the recording, the receiver its copy. `LabRun` (`packet-capture/src/
  lab_run.rs`) loads both, and `bench-transport` fills "Recording delivered": share of the
  recording's bytes arrived identical, and the time to 100 % after the stream ended. The
  baseline has no recording: its column shows "–", and the gate for our transport is absolute:
  100 % on every scenario that has a recording.

## Steps

Each step ends with tests, clippy (`-D warnings`), `cargo fmt`, docs, a commit. One lab run
locally before pushing; the CI's `transport-bench` (3 runs, ~1 h 45) has the last word.

1. Decisions above, in `decisions.md`.
2. Wire: the background payload header in `bonding-protocol`, with its proofs.
3. Receiver: chunks written at their offset; tests with an in-memory recording, chunks out of
   order and repeated.
4. Sender: recording reader, chunks in flight, losses resent; unit tests with a scripted
   sequence of ACKs, an outage of every link in the middle.
5. Lab: the recording writer, `FREEWHEEL_RECORDING*` variables in the scenarios that use our
   transport, the copy compared at the end.
6. Bench: the column filled, the gate on 100 %, the live gates unchanged.
7. Scheduling beyond the probe runs, opened up step by step as long as the live metrics hold.
8. `roadmap.md` (step 6 ✅ with the CI numbers), `freewheel.md` (design and results), PR.

## Lessons from step 5 (save yourself the time)

- **A clean machine for lab runs.** Nothing heavy beside a lab run (`cargo test` of the
  workspace, `du` over the disk): the encoder starves and the run is worthless (70 frames
  instead of 3 600). One local run:
  `RECEIVER=transport SENDER=transport DISCARD_VIDEO=1 CAPTURE_SNAPLEN=96
  LAB_ENCODER_LOG_REPORTS=1 CAPTURES_DIR="$PWD/.cache/<name>/transport-transport/run-1"
  labs/srtla-capture/run-all.sh 1 real-ride trace-drive all-links-drop`, then
  `cargo run --release -p packet-capture --bin bench-transport -- .cache/<name>`.
- **The disk is small.** A CI artifact is 0.3–3 GB, a lab run 30–40 MB per scenario. Delete
  your `.cache/<name>` once its numbers are written down; at 98 % OrbStack crashed.
- **Read the reports, not only the table.** With `LAB_ENCODER_LOG_REPORTS=1`, `<scenario>.marks`
  logs every capacity report and every bitrate change; per-second sizes of `sent.1.frames`
  show where the bitrate went. Every fix of step 5 came from there.
- **Baseline medians to beat** (CI, 3 runs, intact kbit/s / frames lost): `all-links-drop`
  2434 / 209, `real-ride` 2970 / 49, `trace-drive` 1081 / 1721, `link-drop` 3028 / 0 (restored
  link share ≥ 45 %). Step 5 numbers that must not regress: CI 45173685823 and 45194700354
  (`freewheel.md`, "Cross-layer bitrate and capacity probing").
- **Conventions**: code and docs in English, one thing per file (`code-style.md`), every
  decision dated in `decisions.md`, chat with Emilien in French.
