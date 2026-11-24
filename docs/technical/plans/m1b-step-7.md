# Plan: M1b step 7, Noise handshake and AEAD

Handoff for the agent taking step 7 of `roadmap.md` (M1b, "Noise"). Written 2026-11-24 at the
end of step 6. Read this whole page, then `roadmap.md` (M1b), `protocol/freewheel.md` ("Goals",
"Wire format v0", then "Recording upload") and the 2026-11-2x entries of `decisions.md` before
touching code.

## Goal

Every freewheel packet authenticated, every payload encrypted, no open relay: goal 5 of
`freewheel.md`, "Secure by default". A box the server does not know is refused; a packet
altered, forged or replayed is dropped; the live stream and the recording upload keep step 6's
numbers.

## State at handoff

- Step 6 is done and merged: `main` at `dcaed8f`, CI run 45352392251 green. Work on branch
  `m1b-step-7` (from `main`; this plan is its first commit).
- Wire format v0 (`bonding-protocol/src/freewheel/`) is plaintext: a 12-byte header (magic
  `0xF0`, kind, link id, class, session id), then the kind's fields. `freewheel.md` already
  says "Noise (M1b step 7) will wrap everything after the header".
- Session setup today: a link sends Join, the server answers Accept (or Close refused); the
  session id is chosen by the sender. Any link can join any session id: that is the hole.
- SRTLA compatibility (`srtla.rs`) is out of scope: it stays as it is.

## Decisions (Emilien, 2026-11-24, in `decisions.md`)

1. **Crate**: `snow`, the reference Noise implementation in Rust (cryptography only from an
   audited crate, 2026-11-10). Its audit status is checked and cited in `decisions.md` before
   it goes in `Cargo.toml`; if neither `snow` nor its primitives' crates are audited, back to
   Emilien before going on.
2. **Pattern**: `IK`. The box knows the server's static public key in advance, so the handshake
   is one round trip, carried by Join and Accept: starting on time is part of M1b's "done
   when".
3. **AEAD**: ChaCha20-Poly1305 by default, AES-256-GCM as a setting, until the ROCK 5T benchmark
   of M3 decides (2026-11-10).
4. **What stays in clear**: the 12-byte header, authenticated as associated data (the server
   routes on the session and link ids before decrypting). Everything after it is encrypted.
   16 bytes of tag per packet, plus the explicit nonce below.
5. **Keys**: one static key pair per box; the server holds the list of authorised box public
   keys. A box not on the list is refused. Keys can be reset: a new key pair for a box, its old
   public key removed from the list. Every path and key is a setting.
6. **Replay**: a sliding window over the nonce, per session and per direction. Rekeying is a
   later step: write it in `roadmap.md`, do not build it.

## Proposed design (to confirm on the bench)

- **Version.** Encrypted packets are wire format v1: magic `0xF1` (`F`, version 1). The server
  still tells freewheel, SRT and SRTLA apart on the first byte. A v0 packet is refused once
  encryption is on.
- **Nonce.** UDP loses and reorders, and several links carry one session, so the receiver
  cannot count nonces: each transport packet carries its own, 64 bits in clear right after the
  header (also authenticated), from one counter per session and direction, shared by all links.
  `snow`'s stateless transport (`StatelessTransportState`, explicit nonce) fits this. Overhead:
  8 + 16 = 24 bytes per packet; the largest data payload goes from 1 480 to 1 456 bytes, the
  recording chunk from 1 471 to 1 447 (`lengths.rs`, `CHUNK_BYTES` default).
- **Handshake.** Join carries IK's first message (box ephemeral, box static key encrypted, a
  payload: the session id it wants, its first link id), Accept the second. The server checks
  the box's static key against the list before answering; unknown key: Close refused, no state
  kept. A lost Join or Accept is retried as today (a new first message each time; the server
  keeps the last completed handshake per session for a retry that crosses its Accept). A link
  that joins later does not run a new handshake: its Join is a transport packet of the session
  (encrypted, nonce checked), so only the session's holder can add links.
- **Session ownership.** A session id belongs to the box key that opened it: a Join from
  another key for a live session id is refused. An IP change stays a non-event (links still
  belong to the session, not to an address).
- **Replay window.** A bitmap of the last `REPLAY_WINDOW_PACKETS` nonces (default sized for the
  largest reordering between links: 500 ms of queuing at the highest rate, see step 3's bound),
  allocated once per session. A nonce below the window or already seen: dropped, counted.
- **Keys and settings.** Read from the environment by the binaries only, as in step 6:
  - Sender: `FREEWHEEL_PRIVATE_KEY` (box static private key, a file), `FREEWHEEL_SERVER_PUBLIC_KEY`
    (a file), `FREEWHEEL_CIPHER` (`chacha20poly1305` | `aes256gcm`).
  - Receiver: `FREEWHEEL_PRIVATE_KEY` (server's), `FREEWHEEL_AUTHORIZED_KEYS` (a file, one
    box public key per line, a comment allowed), `FREEWHEEL_CIPHER`,
    `FREEWHEEL_REPLAY_WINDOW_PACKETS`.
  - A tool to make a key pair (`freewheel-keygen`, writes the private key file with mode 0600
    and prints the public key): resetting a box is running it again and editing the list.
  - The lab generates a server key and a box key per run (`labs/srtla-capture/`), so a bare run
    needs nothing.
  Each setting gets a line in its type's doc comment (what, unit, default) and in the lab's
  README.
- **Hot path.** Encrypt and decrypt into buffers allocated once; no allocation per packet
  (`zero_allocations` tests, with encryption on). A packet that fails to decrypt costs one AEAD
  check and is dropped, never answered.

## Open (ask Emilien before building)

- **Encryption off.** Steps 5 and 6 proved "off is the previous step to the bit". Here, is
  plaintext v0 kept as a setting (lab comparisons, debugging), or is v1 the only freewheel wire
  format once this step is merged? Recommendation: keep v0 behind an explicit setting, off by
  default in the binaries, so the bench can measure what encryption costs and the step 6 digest
  test still holds with it off.

## Bench

- The live gates against step 6 (CI 45352392251): intact video, frames lost, latency p99,
  restored link share; the recording at 100 % on every scenario that records. The 24 bytes per
  packet are ~1.6 % of a full datagram: expect the intact video to move by about that, not more.
- CPU: a `freewheel_encrypt` benchmark (encrypt and decrypt a full datagram, both ciphers) on
  the CI runner (ARM64); the ROCK 5T numbers come in M3.
- Start: the handshake must still start on time with any single control packet lost (Join or
  Accept), on any link (M1b "done when").

## Steps

Each step ends with tests, clippy (`-D warnings`), `cargo fmt`, docs, a commit. One lab run
locally before pushing; the CI's `transport-bench` has the last word.

1. `decisions.md`: `snow`'s audit and version, the dependency accepted. Settings types (sender,
   receiver, lab) and `freewheel-keygen`, with tests.
2. Wire v1 in `bonding-protocol`: the nonce field, the lengths, the handshake payloads; same
   proofs as v0 (round trips, exact layouts, fuzzing, no allocation). The crypto itself stays
   out of the `no_std` protocol crate: it frames, the sender and receiver seal and open.
3. Handshake: Join / Accept carrying IK, the authorised keys, session ownership, a later link's
   Join; tests: unknown key refused, wrong server key, a lost Join, a lost Accept, a second box
   claiming a live session id.
4. Transport: seal and open every packet, the replay window; tests on the simulated network:
   tampered header, tampered payload, replayed packet, reordering across links within the
   window, the recording copy still byte for byte, the zero-allocation tests with encryption on.
5. Lab: keys generated per run, passed through; `bench-transport` unchanged.
6. Bench: the `freewheel_encrypt` benchmark, then the transport bench against step 6.
7. `roadmap.md` (step 7 ✅ with the CI numbers; rekeying as a later step), `freewheel.md`
   ("Wire format v1", security section, results), then merge into `main` (fast-forward).

## Lessons from steps 5 and 6 (save yourself the time)

- **Run clippy after every test you add**, not only after code: a needless `mut` in a test
  failed CI twice in step 6.
- **A clean machine for lab runs** (nothing heavy beside them; the host is loaded, local runs are
  indicative, CI decides). Delete `.cache/<name>` once its numbers are written down.
- **Read the reports, not only the table**: `LAB_ENCODER_LOG_REPORTS=1`, `<scenario>.marks`, the
  per-second sizes of `sent.1.frames`.
- **The simulated network** (`tests/freewheel_network/`) can cut a link both ways (`set_up`) or
  only its uplink (`set_uplink_lost`): pick the one the scenario means.
- **Conventions**: code and docs in English, one thing per file (`code-style.md`), every
  decision dated in `decisions.md`, chat with Emilien in French.
