# Research 04: security, robustness and operations

Angle: how the freewheel transport stays secure, survives hostile networks and broken servers,
evolves without bricking deployed boxes, and can be observed and verified. Written 2026-11-10.

Markers: **[V]** verified against a cited source, **[I]** inference or design judgement, to be
checked by benchmark or prototype.

## Context

- One box (Radxa ROCK 5T, RK3588: 4x Cortex-A76 + 4x Cortex-A55) on a bike, for years, across
  many countries: local SIMs, CGNAT, frequent IP changes, sometimes DPI and censorship.
- Server: own VPS(s), later possibly multi-tenant for IRL streamers.
- Our UDP multipath protocol (`docs/technical/protocol/freewheel.md`), plus SRTLA compatibility, which has
  no authentication at all.
- Rust, no third-party crates except audited crypto. Box OS later: read-only root, signed A/B
  updates, SSH only through WireGuard.
- Goal 5 of the protocol is "secure by default: authenticated and encrypted, no open relay".
  Goal 1 is "never lose the recording", which turns robustness into a security property:
  availability of the recording matters more than confidentiality of the live picture.

## Problem

A public UDP port on a VPS receives bonded video from a moving box whose addresses change every
few minutes. We must:

1. Authenticate the box cheaply and fast (1 RTT at most, often 0) on links with 100–800 ms RTT.
2. Never let an unauthenticated packet change state (address, session, recording).
3. Not become a DDoS amplifier or an open relay to YouTube/Twitch.
4. Keep working when a country blocks UDP, WireGuard-like fingerprints or everything but a
   whitelist.
5. Survive a server crash or a dead VPS without losing the recording, ideally without the
   viewers noticing.
6. Keep boxes that have not been updated for months talking to a newer server.
7. Protect the live location of a solo traveller.

## Needs

| Need | Why |
|---|---|
| Mutual auth with raw public keys | Closed system: we own both ends, no CA needed |
| Forward secrecy, periodic rekey | Multi-year deployment; a stolen box must not decrypt past traffic |
| Session survives IP change and outages | CGNAT rebinding, SIM swap, tunnels, hours without coverage |
| Cheap rejection of junk | Box CPU is shared with the encoder; server must shrug off scans |
| Replay protection tolerant to multipath reordering | Links differ by hundreds of ms |
| Pluggable outer "carrier" | UDP raw, obfuscated UDP, TCP/TLS 443 fallback |
| Version and capability negotiation | Boxes in the field for months between updates |
| Per-link telemetry | Debug the field from the dashboard |
| Bounded parsers, fuzzed, no panic | Every byte from the Internet is hostile |
| Degraded public location | Physical safety |

## Threat model

| # | Adversary | Capability | Asset at risk | Main mitigation | Residual risk |
|---|---|---|---|---|---|
| T1 | Off-path Internet attacker (scanners, botnets) | Sends spoofed or junk UDP to the server | Server CPU, availability | `mac1` keyed by server public key drops junk before any DH; cookie under load; no amplification (response ≤ request); per-IP and global handshake rate limits | Volumetric DDoS of the VPS link: only the provider or a second receiver helps |
| T2 | On-path network (carrier, hotel Wi-Fi, CGNAT) | Read, drop, delay, reorder, inject, rewrite addresses | Stream integrity, confidentiality, session | AEAD on every packet; address updates only from authenticated, newer packets; path validation before full-rate sending | Can always drop: availability is never guaranteed on one link |
| T3 | State censor with DPI (Russia TSPU, Iran, China GFW) | Fingerprint and block protocols, UDP, SNI, whitelists | Availability of the live | No fixed header bytes; carrier fallback (TCP/TLS 443, CDN WebSocket); store-and-forward recording | Whitelist regimes and shutdowns: live is impossible, recording waits |
| T4 | Thief with the box | Physical access, eMMC dump, keys | Impersonation (stream as Emilien), SSH, config | Revocation from dashboard; box holds no platform stream keys; separate WireGuard key; secure boot later | Until revoked, thief can push video into our pipeline (not directly to YouTube) |
| T5 | Malicious viewer / stalker | Watches stream, map, chat; may DDoS the server IP | Physical safety of Emilien, availability | Delayed and blurred public position; VPS IP never exposed (viewers see YouTube/Twitch only); privacy zones | Video itself shows landmarks: needs stream delay and judgement |
| T6 | Malicious tenant (product mode) | Holds valid box credentials for its own tenant | Other tenants' streams, server resources | Tenant id bound to box key; per-tenant quotas; outputs configured server-side only | Noisy-neighbour bandwidth on a shared VPS |
| T7 | SRTLA client without credentials | Registers groups, floods REG packets, hijacks a group if it sees the id | Server resources, stream | Separate port, rate limits, caps, mandatory SRT passphrase + streamid behind SRTLA | On-path attacker can still disrupt an SRTLA group (protocol limitation) |
| T8 | Compromised VPS | Root on the server | Live, recordings, precise GPS history, box control | Box static key on server is public only; control plane via WireGuard with its own keys; offline root key signs server keys and updates | A compromised receiver sees everything it receives: keep raw GPS minimal |
| T9 | Supply chain | Malicious crate or update | Everything | Zero deps rule; audited crypto only; signed A/B updates with offline key | Audited crates can still have bugs (see snow CVE-2024-58265) |

## Recommendations per area

### 1. Handshake and crypto choice

- **Noise IK, specifically `Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s`, the WireGuard handshake**
  **[V for WireGuard using it]**. Reasons:
  - The box always knows the server's static key (pinned), so IK gives a 1-RTT mutual handshake
    with initiator identity encrypted [V Noise spec].
  - XX costs 1.5 RTT and only buys identity hiding of the responder, which we do not need.
  - TLS 1.3 / QUIC bring X.509, ALPN, session tickets and a much larger parser surface for a
    closed system where we own both ends. TLS only becomes useful as camouflage (see §7) [I].
  - The `psk2` slot is a free extension point: today a per-box PSK (defence in depth if X25519
    breaks); later a post-quantum secret from a hybrid KEM exchange, the way Rosenpass feeds
    WireGuard's PSK, without changing the handshake [V Rosenpass shape, I for us].
- **Library**: `snow` (Rust Noise). Trail of Bits reviewed it (4 engineer-weeks, funded by
  1Password, 1 medium + 1 low + 8 informational findings) [V]. Earlier CVE-2024-58265
  (nonce increment DoS, fixed in 0.9.5) [V] shows why we pin and track RustSec. Alternative: our
  own Noise state machine over audited primitives (`x25519-dalek`, `chacha20poly1305`, `blake2`)
  checked against published test vectors; smaller, but it is protocol code we would own [I].
- **No 0-RTT data in the handshake.** IK message 1 payload is "vulnerable to replay, since
  there's no ephemeral contribution from the recipient" [V Noise spec §7.7]. Put only the
  version, capabilities and a TAI64N timestamp in it, like WireGuard, whose server keeps the
  greatest timestamp per peer and rejects older ones [V WireGuard protocol].
- **"0-RTT across outages" by keeping the session, not by resumption tickets.** A session keeps
  its keys for a long lifetime (proposal: up to 15 min of silence, bounded by rekey rules). A
  link that comes back, or a brand new SIM, joins the existing session by sending a packet
  encrypted with the current keys: zero handshake, zero RTT, and no replay problem because
  the packet counter protects it [I]. Only after full expiry does the box run a new 1-RTT IK.
- **Rekey**: new IK handshake on the best link every 2 min or 2^32 packets, old keys valid during
  overlap, one key-phase bit in the header (WireGuard REKEY_AFTER_TIME semantics: initiator only,
  rate-limited, jittered retries [V]). Noise `Rekey()` alone gives no fresh forward secrecy and does
  not reset the nonce [V spec §11.3], so it is not enough.
- **Per-session keys, per-path nonces.** One key pair per direction per session (cheap, one
  handshake), nonce = `path_id (32 bits) || per-path packet number (64 bits)`, the multipath-QUIC
  approach of putting the path id in the nonce [I from draft-ietf-quic-multipath, check text].
  Each path gets its own replay window (2048 bits, WireGuard-sized [V]), so links with very
  different RTTs never shift each other's window. Per-link handshakes would cost one RTT per new
  SIM for no security gain [I].
- **AEAD: ChaCha20-Poly1305.** The RK3588 A76 and A55 cores both report `aes pmull sha1 sha2`
  [V lscpu dumps], so AES-GCM would be hardware-accelerated and probably faster on the A76 [I].
  But the cost is negligible either way: 20 Mbit/s live + 50 Mbit/s upload ≈ 9 MB/s, a few
  percent of one A55 even in pure software [I, measure with a bench on the board in M3].
  ChaCha is constant-time without hardware, is what snow/WireGuard use, and one cipher means no
  negotiation code. The cipher is fixed by the protocol version: if the bench proves AES-256-GCM
  matters, it arrives as a new version, not as a negotiated option [I].
- **Header protection** (QUIC-style mask over packet number and flags, one extra cipher block per
  packet) to remove fixed bytes for DPI and to hide packet counts per path [I].

### 2. Identity and provisioning of boxes

- Each box generates its X25519 static key on first boot, on the encrypted data partition;
  the private key never leaves the box [I]. RK3588 has a hardware crypto block (upstream driver
  in review 2026 [V]) and OTP for secure boot; use secure boot to protect the OS, not to seal the
  key at first [I].
- **Enrollment**: `freewheel enroll` on the server creates a one-time token (short expiry); the box
  presents it with its public key in its first handshake payload; the server binds key → tenant →
  box id. For Emilien alone: just paste the public key in the server config, like a WireGuard peer.
- **Server keys**: the box pins a set of server static keys (current + next), signed by an
  offline root key that also signs OS updates. Rotation = ship the next key in an update first,
  then switch [I].
- **Revocation**: remove the box key from the allowlist and close its sessions instantly; with
  several receivers, push the allowlist as a signed, versioned file to all of them [I].
- **Separation of keys**: transport key, WireGuard (SSH/control) key and update-verification key
  are three different things; losing one does not give the others [I]. The box never holds the
  YouTube/Twitch stream keys: the server does the restream.

### 3. Anti-amplification, DoS, relay abuse

- **`mac1`** on handshake messages, keyed by a hash of the server public key [V WireGuard]: a
  scanner that does not know the key gets silence, at the cost of one BLAKE2s, no DH.
- **Cookie under load** (`mac2`): cookie = MAC(source IP/port) with a secret rotated every 2 min
  [V WireGuard]. Forces return-routability before any DH.
- **No amplification**: handshake response ≤ initiation size; before a path is validated, the
  server sends at most 3x the bytes it received on it (RFC 9000 §8.1 rule [V]). A CVE in Netty
  (CVE-2026-44894) where any token lifted the 3x limit [V] shows this needs a test, not a comment.
- **Silent by default**: unauthenticated or malformed packets get no reply (no error oracle, less
  scan visibility) [I].
- **Open relay impossible by design**: the server's outputs (YouTube, Twitch, recording storage)
  come from server-side tenant config only. Nothing the box sends can name a destination [I].
- Rate limits: handshakes per IP, per box key, global; sessions per box; bytes per tenant [I].

### 4. SRTLA mode hardening

SRTLA registration (REG1/2/3) carries no credential (details to confirm in `srtla.md`, M1 step 1);
`srtla_rec` is only a proxy to an SRT listener [V irlserver README]. Products that add auth do it
around it (OpenIRL receiver uses an API key, bbox-receiver "built-in authentication") [V].

- Separate UDP port from the native protocol; can be disabled per deployment.
- Caps: groups global and per source IP, links per group, REG1 rate per IP, idle timeout on groups.
- **Authentication lives one layer down**: the SRT listener behind SRTLA requires a per-tenant
  `streamid` and an SRT passphrase (AES). A group that does not produce a valid SRT handshake in
  N seconds is dropped [I].
- IP allowlists are mostly useless for mobile senders (CGNAT, changing IPs); offer them only for
  fixed senders [I].
- Document the residual risk: an on-path observer who sees a group id can register links into it
  and disrupt (not decrypt, with SRT encryption) the stream. Native protocol is the secure path.

### 5. Connection migration and NAT rebinding

- Packets carry a session id (connection id) in clear, rotated per path, never the address
  [I, QUIC model].
- A path's address is updated only by a packet that authenticates and has a packet number higher
  than any seen on that path (QUIC rule); then path validation (challenge/response) before
  lifting the 3x limit [I from RFC 9000].
- Keepalives per idle link, interval learned per carrier (start ~20 s, back off while the
  binding survives): CGNAT UDP timeouts are often below the RFC 4787 two-minute recommendation
  [I, measure on real SIMs].
- **Stateless reset**: the server gives a reset token at handshake, derived from a server secret
  and the session id; after a crash it can tell the box "unknown session" with a packet that
  third parties cannot forge, and the box rehandshakes immediately instead of waiting for a
  timeout [I from QUIC].

### 6. Censorship and DPI resistance

Facts [V]: Russia TSPU blocks QUIC by SNI since at least July 2023, plain WireGuard and OpenVPN
are blocked, and since September 2025 some mobile regions run in whitelist mode. China blocks
QUIC by SNI since April 2024 and has blocked "fully encrypted" (random-looking) TCP traffic with
entropy heuristics (USENIX Security 2023). Iran went to dropping all QUIC Initial packets before
the June 2025 shutdown. Community reports say obfuscated protocols (AmneziaWG, Hysteria2,
VLESS+Reality) still pass in Russia [V, user reports, unverified].

Design consequences [I]:

- **Carrier layer** separate from the protocol: the bonded session runs over one or more carriers
  per link, chosen and probed by the agent:
  1. `udp`: native packets, UDP 443 by default (also configurable), no fixed header bytes,
     randomised padding and optional junk packets before the handshake (AmneziaWG's trick, which
     targets exactly WireGuard's fixed type bytes and sizes [V]).
  2. `tcp-tls`: TLS 1.3 over TCP 443 to a real domain with a real certificate, Noise inside.
     Head-of-line blocking hurts live, but reliable + recording keep flowing.
  3. `cdn-ws`: WebSocket through a CDN (domain fronting is mostly dead, but a CDN-hosted domain
     is hard to block without collateral damage).
- Do not try to imitate QUIC: QUIC Initial packets are decryptable by any observer, so a fake is
  detected; real QUIC is SNI-filtered anyway in RU/CN/IR. Real TLS on TCP is the safer disguise.
- Whitelist mode and shutdowns beat everything: the answer is goal 1, store-and-forward of the
  recording plus outage scenes on the server.
- Legal note: circumvention tools are illegal or grey in some countries; keep the obfuscation
  modes off by default and per-country switchable, decided by Emilien, not automatic [I].

### 7. Multi-receiver failover and server crash recovery

- **Warm standby, client-driven** (not anycast at first): the box holds a signed list of
  receivers, keeps an idle session to receiver B (handshake done, keepalives), and switches live
  traffic when A stops acking on all links for ~2 s [I]. Anycast needs BGP or a provider that
  offers it; worth it only in product mode.
- **Output continuity**: YouTube Live offers a primary and a backup ingest URL; A pushes primary,
  B pushes backup, so a receiver switch is a short glitch, not an ended stream [I, check current
  YouTube docs; Twitch has no equivalent to our knowledge].
- **Recording durability**: the recording is chunked and content-addressed; the server acks a
  chunk only after fsync to durable storage, and the box deletes nothing locally before that ack.
  After a switch, B asks for the missing chunks; A and B reconcile through shared object storage
  later. "Acked" must mean "durable", never "received in RAM" [I].
- **Crash**: systemd restarts the receiver; boxes learn it via stateless reset and rehandshake
  (1 RTT); the restream to YouTube resumes; the outage scene covers the gap.
- QUIC-LB (draft-21, Aug 2025) shows how to encode a server id in connection ids so a stateless
  load balancer can route [V]; reserve bits for that in our session id now, even unused [I].

### 8. Versioning and protocol evolution

- One clear version byte (plus a GREASE range) only in handshake packets; everything else is
  negotiated inside the encrypted handshake payload as a capability list [I].
- Frame types split into "ignore if unknown" and "must understand" ranges (QUIC-style); unknown
  mandatory frame = clean close with a reason code that the dashboard shows [I].
- Send reserved/GREASE values regularly so extension points do not ossify (RFC 8701 idea) [I].
- **Compatibility window**: the server supports the current and previous protocol versions for at
  least 12 months; servers update before boxes; CI runs the pinned binaries of the last N box
  releases against the new server (same pattern as the planned SRTLA interop tests) [I].
- A box that cannot negotiate falls back to SRTLA mode, so it still streams [I].

### 9. Observability

- Per path, exported every second over the existing Unix socket to the agent, then the telemetry
  outbox: RTT (min/smoothed/var), loss, reorder, delivered bitrate, cwnd, queue delay, FEC
  repaired, retransmits, late frames, rebindings, keepalive interval, carrier in use [I].
- Security counters on the server: `mac1` failures, cookies sent, auth failures, replay drops,
  revoked-key attempts, SRTLA groups refused, per source /24 [I].
- **Event traces**: a qlog-like JSON event log (qlog is the IETF schema for QUIC traces), kept in a
  ring buffer on the box and dumped on demand or on incident; traces replay in `link-emulator` [I].
- Server metrics as hand-written Prometheus text format (no dependency needed) [I].
- Never log precise GPS or keys in server logs.

### 10. Fuzzing and formal methods

- **Zero-dep tension**: `cargo-fuzz` needs the `libfuzzer-sys` crate. Proposal: a separate,
  workspace-excluded `fuzz/` directory where `libfuzzer-sys` is allowed as a test-only dependency,
  never linked into shipped binaries; log the exception in `decisions.md` [I].
- In-tree, dependency-free: own deterministic PRNG + structure-aware generators, millions of
  iterations in `cargo test --release` (`decode(encode(x)) == x`, no panic on any byte string,
  bounded allocation) [I].
- **Kani** is a tool (`cargo kani`), not a crate dependency: bounded proofs that parsers never
  panic or overflow for all inputs up to N bytes [V Kani docs, I for fit].
- **Model checking of our state machine**: the Noise IK core is already verified (Noise Explorer
  generates ProVerif models for any pattern [V]); our risk is in the additions (path validation,
  rekey overlap, resumption, failover). Write the session as a pure state machine and add an
  in-house bounded explorer (BFS over events with a dedup set, a few hundred lines, Stateright-style)
  checking invariants: no data accepted before auth; no address change from an unauthenticated
  packet; never more than 3x to an unvalidated path; no nonce reuse across rekey; recording chunk
  deleted on the box only after a durable ack [I]. TLA+ for the failover protocol if it grows.

### 11. Privacy of live GPS

- Precise GPS flows encrypted to the server and stays in private storage [I].
- **Public position is a separate, degraded product**: delayed (default 60 min, configurable),
  blurred (snap to a grid of a few km or to the nearest town), hidden when stationary (camp,
  hotel) and in user-defined privacy zones; the night location is never public until he has left
  [I]. Safety guides recommend 5–10 minute stream delays against stream sniping [V, low-grade source].
- Enforce it in `packages/domain`: a `PublicPosition` value object constructible only through the
  degradation policy, so no public API can leak a raw fix by mistake [I].
- The video also leaks position: optional stream delay, and a "privacy scene" toggle from the
  phone [I].

## Proposed design decisions

1. Native handshake: `Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s` via `snow`, server key pinned, per-box
   PSK in the psk2 slot (reserved for a future post-quantum secret).
2. No 0-RTT handshake data; sessions persist across outages up to 15 min of silence; new links
   join with current keys.
3. One key per direction per session; nonce = path id + per-path packet number; 2048-bit replay
   window per path; full rekey every 2 min, key-phase bit.
4. ChaCha20-Poly1305 only; cipher fixed by protocol version; bench AES-GCM on the board in M3.
5. `mac1` + cookie + 3x anti-amplification + silent drop, each with a dedicated test.
6. Box keys generated on the box; enrollment by one-time token; signed allowlist; instant revocation.
7. Offline root key signs server key sets and OS updates; three separate key families on the box.
8. SRTLA mode: own port, caps and rate limits, mandatory SRT passphrase and streamid behind it.
9. Address changes only from authenticated newer packets, then path validation; stateless reset.
10. Carrier layer: `udp` (443, no fixed bytes), `tcp-tls` (443, real cert), `cdn-ws`; off by
    default except `udp`, per-country switch.
11. Warm standby receiver, client-driven failover; recording acks mean durable (fsync).
12. Version byte + GREASE in handshake only; capability list encrypted; ignorable/mandatory frame
    ranges; server supports N and N-1 for 12 months; CI runs old box binaries against new server.
13. Per-path metrics every second into the telemetry outbox; qlog-like ring-buffer traces.
14. Fuzzing in an excluded `fuzz/` crate (libfuzzer-sys test-only), in-tree PRNG property tests,
    Kani for parsers, in-house bounded model checker for the session state machine.
15. `PublicPosition` type: delayed, blurred, stationary-hidden; raw GPS never public.

## Open questions

- `snow` vs our own Noise state machine on audited primitives: which is less risk over 5 years?
- Do we want post-quantum now (hybrid ML-KEM into psk2) or only the reserved slot? Traffic is
  live video; "harvest now, decrypt later" mostly threatens GPS history and control messages.
- Session silence lifetime: 15 min balances outages vs exposure of keys in RAM; field data needed.
- Is a CDN (Cloudflare) acceptable as fallback carrier, given it terminates TLS?
- YouTube backup ingest behaviour today, and what to do for Twitch/Kick in a receiver switch.
- Stolen box: is a remote wipe over the tunnel worth it, or only revocation?
- Product mode: per-tenant VPS or shared server with quotas? Changes T6 a lot.
- Does secure boot on the ROCK 5T (OTP fuses) fit our "one-command provisioning" goal in M6?

## Sources

- WireGuard protocol and cryptography: https://www.wireguard.com/protocol/
- WireGuard NDSS 2017 slides: https://www.wireguard.com/talks/ndss2017-slides.pdf
- Noise Protocol Framework spec: https://noiseprotocol.org/noise.html
- Noise DoS discussion (Donenfeld, 2015): https://moderncrypto.org/mail-archive/noise/2015/000463.html
- snow audit by Trail of Bits (1Password): https://1password.com/blog/strengthening-snow-for-the-open-source-community
- snow CVE-2024-58265: https://www.tenable.com/cve/CVE-2024-58265
- RustCrypto AEAD audit (NCC Group, 2020): https://www.nccgroup.com/research/public-report-rustcrypto-aesgcm-and-chacha20pluspoly1305-implementation-review/
- RK3588 CPU flags (lscpu): https://openbenchmarking.org/system/2210302-NE-2210305NE40/RK3588-A76-Coremark/lscpu
- RK3588 AES on Rock 5B (forum): https://forum.radxa.com/t/rock-5b-cant-use-aes-accel-how-to/14666
- RK3588 crypto engine driver patches (2026): https://lists.infradead.org/pipermail/linux-rockchip/2026-May/071581.html
- PQNoise: https://eprint.iacr.org/2022/539
- I2P hybrid PQ Noise IK: https://i2p.net/docs/specs/ecies-hybrid/
- Netty CVE-2026-44894 (anti-amplification bypass): https://releasealert.dev/cve/CVE-2026-44894
- QUIC-LB draft-21: https://datatracker.ietf.org/doc/html/draft-ietf-quic-load-balancers-21
- Multipath QUIC draft-21: https://datatracker.ietf.org/doc/draft-ietf-quic-multipath
- GFW detects fully encrypted traffic (USENIX Sec 2023): https://www.usenix.org/conference/usenixsecurity23/presentation/wu-mingshi
- China QUIC SNI censorship: https://upb-syssec.github.io/blog/2025/quic-china
- Russia QUIC SNI censorship (FOCI 2026): https://ntc.party/t/on-russias-early-introduction-of-quic-sni-censorship-foci-2026/25670
- Iran QUIC and UDP blocking (FOCI 2026): https://www.petsymposium.org/foci/2026/foci-2026-0016.pdf
- Russia censorship 2026, mobile whitelists: https://en.zona.media/article/2026/04/07/russian_internet_censorship_2026
- Russia TSPU VPN blocking thread: https://ntc.party/t/%D0%B1%D0%BB%D0%BE%D0%BA%D0%B8%D1%80%D0%BE%D0%B2%D0%BA%D0%B0-vpn-%D0%BF%D1%80%D0%BE%D1%82%D0%BE%D0%BA%D0%BE%D0%BB%D0%BE%D0%B2-%D0%BD%D0%B0-%D1%82%D1%81%D0%BF%D1%83-05082023-xxxx202x/5124/7
- AmneziaWG obfuscation: https://docs.amnezia.org/documentation/amnezia-wg
- irlserver srtla receiver: https://github.com/irlserver/srtla
- OpenIRL srtla-receiver: https://github.com/OpenIRL/srtla-receiver
- Kani vs other tools: https://model-checking.github.io/kani/tool-comparison.html
- Bolero (fuzz/Kani harnesses): https://github.com/camshaft/bolero
- Trail of Bits Rust model checking handbook: https://trailofbits.github.io/testing-handbook-preview/pr-preview/pr-128/docs/languages/rust/lang-rust-model-checking/
- Noise Explorer: https://typeset.io/pdf/noise-explorer-fully-automated-modeling-and-verification-for-3tcoqncib9.pdf
- IRL streamer safety and delay: https://gaminghq.eu/2025/03/13/irl-streamers-face-growing-safety-risks-heres-how-to-stay-safe-while-streaming-in-public
