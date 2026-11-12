# 01 — State of the art and competitors

Research date: 2026-11-10. Angle: what exists today, what it does well, and what gaps freewheel's own transport can fill.

Legend: **[V]** verified from a primary source (spec, official repo, paper abstract, vendor doc). **[C]** vendor or third-party claim, not independently verified. **[I]** my inference.

## Context

freewheel streams from a battery-powered RK3588 box (H.265, 1080p30) over 1–4 cellular links with cheap local SIMs, plus Wi-Fi when available, to our own VPS, which relays to YouTube/Twitch. The trip runs for years, outages are normal, and the full-quality recording must never be lost. We stay wire-compatible with SRTLA (BELABOX) as a mode and as the benchmark baseline, and we build our own protocol (`docs/technical/protocol/freewheel.md`).

## Problem

Bonded live contribution splits into two worlds:

- **Proprietary broadcast bonding** (LiveU, TVU, Dejero, Haivision, Zixi): mature, media-aware, sub-second latency claims, but closed, patented, sold with hardware and a recurring cloud service.
- **Open IRL bonding** (SRTLA from BELABOX, Moblin, IRL Pro, srtla_send forks): free and self-hostable, but media-blind. It has no FEC, no encryption, no notion of data cost, and it takes multi-second buffers.

Standards bodies are converging on QUIC (Multipath QUIC, MoQ), but nothing there targets bonded cellular contribution with a recording backfill yet.

## Needs (from the freewheel goals)

1. Recording reaches the server eventually, 100 % (store-and-forward integrated with live).
2. Best picture for the aggregate capacity, using every link.
3. Low, stable latency (p99 matters more than p50).
4. Survive churn: IP change, SIM swap, link death mid-frame, long outages.
5. Secure by default: authenticated, encrypted, no open relay.
6. User priorities: data cost per SIM, battery, latency vs quality.
7. Fits a ~10–15 W box and ARM CPU; open source, self-hosted, MIT-compatible.

## Landscape

| Tech | Kind | Multipath | Loss recovery | Media-aware | Crypto | Typical latency | Openness / licence |
|---|---|---|---|---|---|---|---|
| SRT | Point-to-point UDP | Socket groups: broadcast, main/backup [V] | ARQ (NAK), optional FEC filter | No | AES passphrase | 120 ms default buffer [V]; ~4×RTT guidance [C] | MPL-2.0, libsrt v1.5.7 (2026-08) [V]; IETF draft expired 2021 [V] |
| SRTLA | Proxy that sprays SRT packets over N links | Yes, per-link in-flight window [V] | Relies on SRT ARQ | No | None [V] | Example config 2000 ms [V]; 2–3 s used in IRL [C] | AGPL-3.0 [V]; receiver "unsupported" [V] |
| RIST | RTP-based profiles (VSF TR-06-1/2/3) | Bonding = 2022-7 duplication or load sharing [V] | ARQ on any link, 2022-1 FEC | No | DTLS/PSK (Main) | Configurable | Open spec, libRIST BSD-2 |
| Zixi | Proprietary | Bonding over 5G/LTE/internet [V] | Hybrid adaptive FEC + ARQ [V] | Partly | AES | Latency ≥ 3×RTT; 500 ms interactive, 4 s default [V]; "<300 ms" [C] | Closed, SDK licence |
| LiveU LRT | Proprietary, cloud-bonded | Yes | Dynamic FEC + ARQ + ABR [C] | Yes [C] | Yes | 0.4–1 s [C]; ~5 s in Solo spec [C] | Closed, per-unit subscription |
| TVU IS+/ISX | Proprietary "Inverse StatMux" | Yes, incl. Starlink | Proprietary | Yes [C] | Yes | "0.3 s on cellular" [C] | Closed, patented [C] |
| Dejero Smart Blending | Proprietary | Packet-level blending of cellular/sat/Wi-Fi [V] | Proprietary | Partly | Yes | Sub-second [C] | Closed, patented [C] |
| Haivision SST (Pro460) | Proprietary | Yes | ARQ + ABR [V] | Yes | Yes | 0.2–0.5 s; 80 ms on private 5G [C] | Closed, patented [V] |
| Peplink SpeedFusion | Router VPN bonding | Bonding, hot failover, WAN smoothing (duplication) [V] | FEC + duplication [V] | No (per-flow) | Yes | Penalised by the slowest link's reordering [C] | Closed, licensed per router |
| Speedify | Consumer bonding VPN | Speed / redundant / streaming modes [V] | Duplication when lossy [V] | Detects real-time flows [V] | Yes | Not published | Closed, subscription |
| MPTCP | Kernel TCP extension | Yes | TCP retransmit | No, HoL blocking [V] | TLS above | Bad for live | Open (Linux), RFC 8684 |
| Multipath QUIC | IETF draft-21 | Yes, path IDs | QUIC ARQ; datagrams unreliable | No, scheduler left to app | TLS 1.3 | Depends on scheduler | IESG-approved, RFC Editor queue (2026-05) [V/I] |
| MoQ (MoQT) | Pub/sub media over QUIC | No (single path today) | Drop by group/priority | Yes (tracks, groups, priorities) | TLS 1.3 | Sub-second | IETF draft-19 (2026-07) [C]; Cloudflare relay on draft-07 [C] |
| WebRTC / WHIP | RTP/SRTP, ingest RFC 9725 [V] | No (Converge research adds it) | NACK, RED, FlexFEC | Yes (SVC, simulcast) | DTLS-SRTP | <500 ms | Open; RFC 9725 March 2025 [V] |
| RTMP / E-RTMP | TCP | No | TCP | No | RTMPS | 2–5 s | Open; E-RTMP v2 adds HEVC/AV1, multitrack, reconnect [V] |
| SMPTE 2022-1 / 2022-7 | Row/column XOR FEC / dual-path duplication | 2022-7 duplication only [V] | FEC 2022-1 | No | No | Managed networks | Standards, used inside RIST |

## Per-technology notes

**SRT** [V]: Haivision's ARQ transport, the de facto contribution protocol, MPL-2.0. Socket groups give broadcast (send all packets on every link, receiver dedups) and main/backup (switch on instability, latency at least 2× the stability timeout, ~100 ms cost in their tests). It has no load-sharing bonding for cellular. Weaknesses for us: a fixed latency buffer, media-blind, passphrase AES with no identity and no forward secrecy [I], and a session that dies on long outages [I].

**SRTLA (BELABOX)** [V]: a proxy that registers N links into one group (REG1/2/3), tracks in-flight packets per link with a TCP-like window, and uses SRT NAKs to penalise links. "No congestion control is applied"; bitrate adaptation lives in belacoder (SRT send buffer, RTT). No authentication, no encryption. `srtla_rec` is "unsupported, not suitable for production". **License correction: `srtla` is AGPL-3.0 and `belacoder` is GPL-3.0** (freewheel README says "GPL-3.0" for all of upstream). The clean-room policy still holds. Ecosystem forks: CERALIVE/irlserver receiver (ACK throttling, quality weights 10–100 %), **irlserver/srtla_send, an MIT Rust sender** (NAK decay, RTT bonus, quality scoring, stalled-link deselect for Starlink). That is the closest open-source prior art to our `bonding-sender` and the bar our SRTLA mode must clear, not just upstream C.

**RIST** [V]: VSF profiles. Simple (TR-06-1, 2018) already defines bonding as 2022-7 duplication or load sharing with ARQ on any link. Main (TR-06-2) adds tunnelling, auth and encryption; Advanced (TR-06-3, updated 2024) adds tunnel-level ARQ. Moblin bonds over RIST by default; Larix supports RIST Main. It is technically better specified than SRTLA, but IRL adoption is low [C], and its weighted load sharing is not designed around cellular churn [I].

**Zixi** [V]: hybrid FEC + ARQ whose FEC block latency adapts to loss, RTT and jitter. Below 1.5 s latency its FEC overhead reaches ~30 %. It is the clearest public statement of the latency vs overhead tradeoff. Closed.

**LiveU LRT / TVU ISX / Dejero / Haivision SST**: all claim per-packet scheduling across links, dynamic FEC, ARQ and encoder-coupled ABR, with sub-second latency (TVU 0.3 s, LiveU 0.4–1 s, Haivision 0.2–0.5 s) [C]. None publishes a spec or an independent benchmark. All are patented [C] (Haivision says "patented"; TVU says "patented Inverse StatMux"). LiveU has **Store & Forward**: record on the unit, then forward the file automatically, in 3 quality levels [V]. It is a separate mode, not a background upload that runs concurrently with live [I]. Business model: hardware plus a per-unit yearly cloud bonding licence [V].

**Peplink SpeedFusion / Speedify** [V]: generic IP bonding VPNs. Useful ideas: WAN smoothing (duplicate on the link with stable latency at that moment), FEC, hot failover, and Speedify's per-connection **priority (Primary / Secondary / Backup) plus daily and monthly data caps with overlimit behaviour**. They are the only products that are explicitly cost-aware, but only at the connection level, not per traffic class or per frame [I]. Peplink warns that bonding links with very different latencies penalises everything through the reordering buffer.

**MPTCP**: open and in Linux, but TCP head-of-line blocking and reordering delay make it a poor fit for live video. The kernel ships only default (lowest RTT) and round-robin schedulers [C]. Discard it for the live class; it could fit the background upload class [I].

**Multipath QUIC** [V]: draft-ietf-quic-multipath-21 (March 2026), approved and awaiting RFC editor since 2026-05. Path IDs make NAT rebinding and IP change first-class. Rust implementations: **noq** (n0/iroh, a Quinn hard fork with full multipath, interop-tested against picoquic, March 2026) [C]. The scheduler is left to the application, so the hard part (media-aware, deadline-aware scheduling) is still ours [I]. Cost: TLS 1.3 + QUIC stack in Rust as a large dependency against our zero-crate rule.

**MoQ** [C]: pub/sub over QUIC, media modelled as tracks, groups and objects, with priorities and drop-by-group. Moving fast (draft-16 to draft-19 between January and July 2026); Cloudflare's relay network (launched 2025-08-22) speaks draft-07, so interop is fragmented. moq-lite (kixelated, Apache-2.0) is a pragmatic subset. Its object model (group = GOP, priority per track, relays can serve past groups) is the right **media semantics** to borrow. It is not multipath, and YouTube/Twitch do not ingest it [I].

**WebRTC / WHIP** [V]: RFC 9725 (March 2025) standardises HTTP ingest. It has the richest loss tools (NACK, RED, FlexFEC, SVC) and the lowest latency, but it is single-path. Moblin streams WHIP. Twitch accepts WHIP [C]; YouTube does not by default [I].

**E-RTMP v2** [V]: Veovera spec adding HEVC/AV1, multitrack and a reconnect request. That matters only for our server → platform egress, not for contribution.

**Mobile apps**: Moblin (iOS, MIT, very active) offers RTMP/SRT/SRTLA/RIST/WHIP, bonding over cellular + Wi-Fi + Ethernet, **Moblink** (other phones as extra links), adaptive bitrate and local MP4 recording [V]. IRL Pro (Android) speaks SRTLA [V]. Brix (F-Droid) is an open SRTLA Android sender [V]. Larix offers "Wi-Fi + cellular bonding" with an undocumented protocol [V/C]. All of them record locally; none backfills the recording to the server [I].

**Academic work (2023–2026)** [V, from abstracts]:
- **CellFusion** (SIGCOMM 2023, Alibaba): multipath QUIC + network coding (XNC) over several carriers for vehicle-to-cloud video, 100 vehicles over 6 months. p99 video packet delay −71.5 % vs 5G; stall ratio −66 % to −81 % vs state-of-the-art multipath, at <10 % redundancy. **This is the closest published analogue to freewheel.**
- **Converge** (SIGCOMM 2023): naive multipath WebRTC can do *worse* than single path. A video-aware scheduler, receiver QoE feedback and per-path protection give ~1.2× throughput, −20 % latency, +55 % quality. Built into Chromium.
- **STORM** (USENIX ATC 2025): MPQUIC scheduler that separates reliable from unreliable traffic so retransmissions never block live data. Tail delay −98.2 %, frame rate 1.95×. Latency is dominated by the last-mile wireless link.
- **AUGUR** (NSDI 2024): multipath for low tail latency that uses cellular only when needed, **keeping cellular data usage constrained**, i.e. cost-aware.
- **LoLa** (arXiv 2023): multi-carrier cellular video conferencing, link-quality-driven packet split.
- **Tambur** (NSDI 2023): streaming codes for variable-size video frames, −26 % decode failures and −35 % FEC bandwidth vs block FEC on Teams traces.
- **Compact** (MMSys 2025): content-aware multipath (foreground/background) over cellular.

**FEC licensing**: RFC 8681 sliding-window RLC (GF(2) and GF(2^8)) is the IETF standard for real-time FEC with lower latency than block codes [V]. RaptorQ (RFC 6330) is Qualcomm-owned with IPR disclosures [I, check datatracker]. RLNC has commercial patent holders (Steinwurf / Code On) [I]. Simple XOR (2022-1) and RFC 8681 RLC are the safe defaults [I].

## Gaps nobody fills well

1. **Live and recording in one transport.** Everyone treats the recording as either local-only (all IRL apps, BELABOX) or a separate store-and-forward mode (LiveU). Nobody runs a scavenger-priority backfill *concurrently* with live that resumes across outages and patches the VOD's holes. [I, high confidence]
2. **Data cost per SIM.** Bonding products split by capacity and quality; only generic VPNs (Speedify, Peplink) expose priorities and caps, and only per connection. Nobody schedules per traffic class (audio and keyframes on the expensive SIM, background upload only on Wi-Fi or the cheap SIM) or budgets € per GB per day. AUGUR shows the research direction. [I]
3. **Media-aware scheduling in open source.** SRTLA and srtla_send are media-blind: they see SRT packets inside MPEG-TS. No open stack duplicates audio and keyframes, drops B-frames under pressure, or retransmits only what can still meet its deadline. Converge and STORM prove 1.2–2× gains. [V for the papers, I for the gap]
4. **FEC in open IRL bonding.** None; everything waits for an ARQ round trip, which is why IRL buffers are 2–3 s. CellFusion gets large tail gains at <10 % redundancy. [V/I]
5. **Long outages as a first-class state.** SRT sessions time out, the platform ends the broadcast, and relays fake "BRB" scenes. Nobody keeps one session alive across a 30-minute tunnel and resumes instantly with the backlog prioritised. [I]
6. **Security.** SRTLA has none, SRT uses a shared passphrase. No open bonding stack has device identity, forward secrecy, or anti-amplification by default. [V for SRTLA, I otherwise]
7. **Energy as a scheduling input.** Duplication and keeping 4 modems hot cost battery (Speedify warns about it). Nobody trades joules per bit against quality on a battery box. [I]
8. **Open, reproducible benchmarks.** Every latency number above is a vendor claim. There is no public benchmark of bonding protocols on recorded cellular traces. [V: none found]
9. **Self-hosted receiver quality.** BELABOX calls its own receiver unsupported and pushes its cloud; proprietary vendors require their cloud. [V]

## Opportunities for freewheel, ranked by impact

1. **Live + background recording backfill in one session** (gaps 1, 5). Unique in the market and directly serves goal #1. The server stitches the full-quality recording into the replay after an outage. It is the headline feature for a product too ("your stream had a hole, your VOD doesn't").
2. **Cost- and battery-aware policy per traffic class** (gaps 2, 7). Per SIM: € per GB, daily cap, priority; per class: which links it may use. A "budget mode" for local SIMs is something no bonding product offers and IRL streamers abroad need. [I]
3. **Deadline-aware, media-aware scheduler** (gap 3): earliest-arrival per packet, priority by frame type, audio and keyframe duplication, retransmit only if it beats the deadline (STORM-style separation of the live and reliable classes). This is the core of beating the SRTLA baseline on lost frames at equal latency.
4. **Adaptive sliding-window FEC** (gap 4), RFC 8681-style RLC sized to measured loss per link, aiming at CellFusion-like tail gains at <10 % overhead. Patent-safe if we stay on RLC/XOR. [I]
5. **Outage-proof sessions** (gap 5): session IDs (like MPQUIC path IDs), the server holds the platform output with an outage scene, and live resumes in under a second when a link returns.
6. **Secure by default** (gap 6): Noise handshake, per-device keys, no unauthenticated reply larger than the request. Cheap to do and a real differentiator against SRTLA.
7. **Public benchmark suite and traces** (gap 8): the M1/M1b bench published with recorded cellular traces from the trip. It gives credibility for a product and helps the community. Compare against SRTLA upstream, **irlserver/srtla_send**, RIST bonding and, if possible, Moblin.
8. **Borrow MoQ semantics, not MoQ** (tracks, groups = GOP, priorities, drop-by-group) so a future MoQ egress or interop is trivial; keep the wire our own until MoQT stabilises. [I]
9. **Keep Multipath QUIC (noq) as the comparison candidate**, not the base: it solves path management, but scheduling, FEC and the recording class would still be ours, at the price of a large dependency. Re-evaluate once the RFC is published. [I]

## Sources

- SRTLA (AGPL-3.0, README): https://github.com/BELABOX/srtla
- belacoder (GPL-3.0): https://github.com/BELABOX/belacoder
- irlserver Rust sender (MIT): https://github.com/irlserver/srtla_send
- CERALIVE srtla fork: https://github.com/CERALIVE/srtla
- SRT socket groups: https://sources.debian.org/src/srt/1.5.4-3/docs/features/socket-groups.md
- SRT main/backup: https://github.com/jeandube/srt/blob/master/docs/features/bonding-main-backup.md
- SRT draft: https://datatracker.ietf.org/doc/draft-sharabayko-srt/
- SRT releases: https://github.com/Haivision/srt/releases
- RIST bonding: https://www.rist.tv/articles-and-deep-dives/2020/6/16/rist-bonding-and-load-sharing-demystified
- RIST multi-link: https://www.rist.tv/articles-and-deep-dives/2022/5/9/multi-link-operation-using-rist
- VSF RIST: https://vsf.tv/RIST.shtml
- OBE on 2022-7 over the internet: https://www.obe.tv/?p=14195
- Zixi docs: https://zixidocumentation.atlassian.net/wiki/x/HgB5L
- LiveU LRT: https://www.liveu.tv/lrt
- SVG bonded 5G roundtable (LiveU latency): https://www.sportsvideo.org/2025/04/01/tech-trends-roundtable-bonded-5g-cellular-ip-video-production-part-2-systems-service-suppliers-weigh-in/
- LiveU Store & Forward guide: https://nccnewslinks.syr.edu/wp-content/uploads/2022/11/LiveU-Guide.pdf
- TVU ISX: https://www.sportsvideo.org/2023/09/08/tvu-networks-unveils-isx-next-gen-inverse-statmux-statistical-multiplexing-technology/
- Dejero: https://dejero.com/company/why-dejero
- Haivision SST: https://haivision.com/products/sst-protocol/
- Haivision Pro460: https://doc.haivision.com/4_series/4.0/Pro460/product-presentation
- Peplink SpeedFusion whitepaper: https://www.venntelecom.com/documents/13/SpeedFusion_Whitepaper_ver04-02.pdf
- Speedify streaming mode: https://support.speedify.com/article/692-streaming-mode
- Speedify priorities and data limits: https://support.speedify.com/article/344-data-limits
- Multipath QUIC: https://datatracker.ietf.org/doc/draft-ietf-quic-multipath/
- noq: https://iroh.computer/blog/noq-announcement
- MoQ transport: https://datatracker.ietf.org/doc/draft-ietf-moq-transport/
- MoQ draft-19 / Cloudflare draft-07: https://www.youngju.dev/transcribe/2026-07-17-moq-transport-draft19-cloudflare-relay-moq-lite.en
- First MoQ CDN: https://moq.dev/blog/first-cdn/
- WHIP RFC 9725: https://www.rfc-editor.org/rfc/rfc9725
- Enhanced RTMP v2: https://veovera.org/docs/enhanced/enhanced-rtmp-v2
- Moblin: https://github.com/eerimoq/moblin
- IRL Pro setup: https://kb.irltoolkit.com/cloudobs/2-Encoders/03-IRLPro
- Brix: https://f-droid.org/packages/app.brix/
- Larix: https://softvelum.com/larix
- CellFusion (SIGCOMM 2023): https://yfmascgy.github.io/papers/cellfusion.pdf
- Converge (SIGCOMM 2023): https://experts.colorado.edu/display/pubid_360080
- STORM (ATC 2025): https://www.usenix.org/conference/atc25/presentation/hu-liekun
- AUGUR (NSDI 2024): https://www.usenix.org/system/files/nsdi24spring_prepub_zhou-yuhan.pdf
- LoLa: https://arxiv.org/abs/2312.12642
- Tambur (NSDI 2023): https://www.usenix.org/conference/nsdi23/presentation/rudow
- Compact (MMSys 2025): https://faculty.iiitd.ac.in/~arani/assets/pdf/mmsys25.pdf
- MPTCP + FEC: https://arxiv.org/pdf/1807.11059
- RFC 8681 sliding-window RLC FEC: https://www.rfc-editor.org/rfc/rfc8681
- RFC 6330 RaptorQ: https://www.rfc-editor.org/rfc/rfc6330
