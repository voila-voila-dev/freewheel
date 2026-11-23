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

- Step 5 is done and merged: `main` at `7ad1e8c`, CI run 45194700354 green (its two flaky
  characterisation tests, `group_limit` and `srt_server_restart`, passed on rerun). Work on
  branch `m1b-step-6` (from `main`; this plan is its first commit).
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

## Decisions (Emilien, 2026-11-23, in `decisions.md`)

1. **What the lab records**: a file that grows during the run at a constant 6 Mbit/s (2× the
   live ceiling, as a camera's full-quality file would), random bytes, compared byte for byte
   with the receiver's copy. The real box later points the sender at the encoder's segments.
2. **Drain time**: after the stream ends, the links stay up as long as the scenario took; the
   bench reports the time to 100 %, and judges 100 % at the end of it.
3. **Share**: only what the live class leaves, never queuing the live traffic. Latency p99 and
   intact video must not regress against step 5 (CI 45194700354).
4. **Resume**: across link outages, the sender and the receiver running all along. A restart
   of either (the box reboots) is a later step: write it in `roadmap.md`, do not build it.
5. **Everything is a setting**: the recording can be switched on and off, and every number of
   the upload is configurable, nothing hard-coded (see "Settings" below).

## Settings

One settings type per side (sender, receiver), with documented defaults, built from the
environment in the binaries (`freewheel_sender.rs`, `freewheel_receiver.rs`, as
`FREEWHEEL_CAPACITY_SOCKET` and `FREEWHEEL_MAX_VIDEO_BPS` are today), never read deep in the
code. At least:

- **On/off**: no recording setting, no upload, and the sender and receiver behave exactly as in
  step 5 (padding for probing included): the step 5 bench numbers must hold with it off.
- **Sender**: the recording's path; the upload's share of the spare capacity (cap, in per
  cent); chunk size; loss timeout; how long to keep trying after the stream ends.
- **Receiver**: on/off, the directory the copies are written to.
- **Lab**: the recording's rate (default 6 Mbit/s), the drain time, on/off per scenario, through
  the lab's variables (`labs/srtla-capture/`), with defaults so a bare run stays as today.

Each setting gets a line in its type's doc comment (what, unit, default) and in
`labs/srtla-capture/README.md` for the lab's. Tests cover "off" as well as "on".

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

1. Settings types (sender, receiver, lab), off by default, with tests that "off" is step 5.
2. Wire: the background payload header in `bonding-protocol`, with its proofs.
3. Receiver: chunks written at their offset; tests with an in-memory recording, chunks out of
   order and repeated.
4. Sender: recording reader, chunks in flight, losses resent; unit tests with a scripted
   sequence of ACKs, an outage of every link in the middle.
5. Lab: the recording writer, `FREEWHEEL_RECORDING*` variables in the scenarios that use our
   transport, the copy compared at the end.
6. Bench: the column filled, the gate on 100 %, the live gates unchanged, with the recording
   on and off.
7. Scheduling beyond the probe runs, opened up step by step as long as the live metrics hold.
8. `roadmap.md` (step 6 ✅ with the CI numbers; the restart resume as a later step),
   `freewheel.md` (design and results), then merge into `main` (fast-forward, as steps 1–5).

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
