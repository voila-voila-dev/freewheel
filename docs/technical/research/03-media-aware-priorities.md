# 03 — Media awareness, cross-layer design and user priorities

Research note, 2026-11-10. Angle: how the transport, the encoder and the user's intent work as one system.
Labels: **[V]** verified in a source (linked below), **[S]** vendor-stated, not independently checked, **[I]** our inference or proposal.

## Context

- freewheel sends three traffic classes over 1–4 links: live (deadline), reliable (telemetry/control), background (recording upload). Goal #1 is "never lose the recording". Goal #2 is "best picture for the network we have" (`docs/technical/protocol/freewheel.md`).
- Pipeline: Osmo Action UVC MJPEG 1080p30 → RK3588 MPP H.265 → bond sender → own server → YouTube/Twitch. `bitrate-control` is a pure `no_std` crate that currently only clamps bounds.
- The owner wants **each user to choose their own priority**. That covers him on the bike, and later IRL streamers.
- Outages are content. The server already plans outage scenes, bets and replays.

## Problem

Today's stacks (BELABOX, SRT, SRTLA) treat video as opaque bytes. **[I]** That has five consequences:

1. **Losses hit random packets.** One lost packet in a reference frame corrupts every frame until the next IDR. The usual fix is a big IDR, which costs most exactly when the link is weakest.
2. **The encoder learns about capacity late.** belacoder infers it from SRT send-buffer growth and RTT. By the time it reacts, the queue is already full.
3. **There is one fixed trade-off.** Nobody can say "spend less data in Switzerland", "keep the voice at all costs" or "I'd rather see 15 fps sharp than 30 fps mush".
4. **The recording is an afterthought.** It is either the same degraded stream or a separate manual job.
5. **The server is blind to meaning.** It can't tell "the rider pressed highlight", "outage likely in 20 s" or "these 40 s are missing and will arrive later".

## Needs

- N1. Protect what matters most to the viewer: audio, then decodability, then smoothness, then sharpness.
- N2. React within a frame (33 ms) to capacity drops, not within seconds.
- N3. Recover from loss without an IDR whenever possible.
- N4. One readable policy that the user picks (preset) or tweaks, covering quality, latency, data cost, battery and privacy.
- N5. Per-SIM data budgets with pacing, plus cost-aware link use.
- N6. A recording that is complete, segmented, resumable and smartly ordered.
- N7. Metadata (GPS, speed, heart rate, markers) on the same clock as the media, so the server can build features on it.
- N8. Measured perceived quality that drives decisions and benchmarks, not just raw bitrate.

## What the RK3588 encoder can actually do (key finding)

These are read from the Rockchip MPP headers on the `develop` branch (`inc/rk_venc_ref.h`, `inc/rk_venc_cmd.h`, `inc/mpp_meta.h`, `inc/rk_mpi_cmd.h`) **[V]**:

| Capability | MPP API |
|---|---|
| Temporal SVC presets | `REF_TSVC1/2/3`, plus `tsvc2/3/4` reference configs in `mpi_enc_utils.c`, `max_tid`; per-frame output `KEY_TEMPORAL_ID` |
| Long-term reference frames | Up to 16 (`MPP_ENC_MAX_LT_REF_NUM`), auto LTR every `lt_gap` frames; per frame `KEY_ENC_MARK_LTR` / `KEY_ENC_USE_LTR` |
| Force IDR | `MPP_ENC_SET_IDR_FRAME`, per-frame `KEY_INPUT_IDR_REQ`; output `KEY_OUTPUT_INTRA` |
| Intra refresh | `intra_refresh_mode` (MB row / column / gap) in codec config; `refresh_en/mode/num` in rate control |
| ROI | `MPP_ENC_SET_ROI_CFG` (rectangles with QP delta or forced intra), H.265 up to 64 regions, `KEY_QPMAP0` QP maps, NPU object flags |
| Encoder telemetry per frame | `KEY_ENC_SSE` (gives PSNR for free), `KEY_ENC_AVERAGE_QP`, `KEY_ENC_BPS_RT`, motion vectors (`KEY_MOTION_INFO`) |
| Frame skip | `KEY_INPUT_PSKIP` / `KEY_INPUT_PSKIP_NON_REF` (encode a near-free skip frame) |
| Slices, SEI, user data | `MPP_ENC_SET_SPLIT`, `MPP_ENC_SET_SEI_CFG`, `KEY_USER_DATA` |
| Smart P | Background-referenced GOP (`gop_mode` smart P, `vi_len`) for static scenes |

The GStreamer element (`gstreamer-rockchip` `mpph265enc`) exposes only `bps`, `bps-min/max`, `gop`, `rc-mode`, QP bounds, `header-mode`, `sei-mode`, `rotation` and `width/height`. It supports a forced keyframe and runtime property changes (`prop_dirty`). It has **no** TSVC, LTR, ROI or intra-refresh controls **[V]**.
Whether each feature works on the RK3588's VEPU580 (rather than another Rockchip encoder) still has to be tested on the bench **[I]**.

**Consequence [I]:** the `encoder` crate should drive MPP directly through a small FFI crate. GStreamer would stay for capture and MJPEG decode, or MJPEG decode could also go through MPP. Otherwise the most valuable cross-layer features are out of reach. This argues for "own FFI" in the open GStreamer decision.

## Ideas (ranked by value ÷ effort)

1. **Temporal layers plus importance tags on every packet.** Use TSVC3 (L1T3): TL0 at 7.5 fps, TL1 adds 15 fps, TL2 adds 30 fps. Higher-layer frames are never referenced by lower layers, so dropping them can't corrupt anything **[V for the structure, I for the gain]**. The scheduler drops TL2 first, then TL1. Frame rate degrades gracefully in ~0 ms with no encoder round trip. This is the cheapest big win.
2. **Audio is sacred.** Encode the mic audio as Opus on the box, around 32–64 kbps. Send it on the two best links, with in-band FEC (LBRR). Optionally add DRED, which covers up to ~1 s of burst loss for roughly 12–32 kbps extra **[V]**. DRED isn't standardized yet and the 1.5 bitstream will change **[V]**, which is acceptable because we own both ends. The server transcodes to AAC for RTMP **[I]**. People forgive frozen video, not broken voice.
3. **LTR-based loss recovery instead of IDR.** The receiver acknowledges frames. On an unrecoverable loss the sender sets `KEY_ENC_USE_LTR` on the next frame, so it refers to the last LTR the server confirmed. That costs one slightly larger P frame instead of an IDR that can be 5–10× bigger **[I]**. The IETF draft `avtcore-frame-acknowledgement` (July 2026) standardizes exactly this pattern for WebRTC **[V]**. We copy the idea, not the wire format.
4. **The server is a production switcher, not a relay.** The server decodes and re-encodes toward YouTube/Twitch. This gives:
   - a stream that YouTube can always decode, whatever we lost upstream;
   - frame-accurate outage transitions;
   - overlays (map, speed, heart rate) rendered server-side, so they cost nothing on the box;
   - instant replay;
   - freedom to use LTR and TSVC tricks a third-party ingest might not accept.

   Cost: a few VPS cores for 1080p30 x264/x265 **[I]**. The planned outage scenes and replays already need compositing, so this design makes that explicit.
5. **Dual encoding: live and record.** The record encoder is fixed at high quality: constant-QP or capped VBR, a closed 2 s GOP, never adapted to the network. The live encoder is fully adaptive. Two 1080p30 sessions are far below RK3588's stated 8K encode capacity **[S]**. A single encoder would bake every live degradation into the recording, which breaks goal #1.
6. **The policy is a small declarative document plus presets.** The UI shows 5 big buttons, and advanced users edit the document. See the design below.
7. **Data budget pacer per SIM.** A token bucket sets the daily allowance from quota, renewal date and price per GB. That allowance sets the live bitrate ceiling and gates background upload.
8. **Cost-tiered link roles.** Wi-Fi and cheap SIMs carry everything. An expensive SIM is a "lifeboat": it carries audio, TL0, keyframes and control only, unless the live quality would fall below the user's floor.
9. **Outage backfill.** When a link comes back, upload the recording segments that cover the outage first, at a mid-quality proxy. The server can then play "what you missed during the outage" as content, minutes later. This turns goal #1 into a show feature.
10. **Handlebar highlight button.** The press becomes a marker in the reliable class. The server cuts the live clip at once. The box then prioritizes the high-quality segments from −30 s to +10 s around the marker in the upload queue.
11. **Outage forecast.** RSRP/SINR trends, all-link capacity collapse, and maybe a coverage map ahead on the route give "outage likely in ~N s". The forecast goes in-band to the server, which pre-rolls a transition instead of a hard freeze **[I]**.
12. **Final degradation rungs beyond video.** At ~150 kbps send "audio + one IDR every 2–4 s" (slideshow mode). At ~50 kbps send "audio + live map". Below that, the outage scene with live telemetry over SMS or the reliable class. The stream never goes black.
13. **Content-aware ladder shape.** MPP motion vectors and SSE tell "riding fast" from "standing and talking". On motion the ladder keeps fps and drops resolution. On a static talk it drops fps, keeps resolution, and can switch to smart-P **[I]**.
14. **Face/centre ROI.** For "Talk show", spend bits on a centre or face rectangle via ROI or the QP map, using the RK3588 NPU for face detection later. Lower priority, because an action camera rarely frames a face reliably.
15. **Multi-camera phone apps.** A phone joins the same session as an extra camera or extra link. All sources share the session clock, and the server switches angles. This matters for the IRL product, not for the solo trip.

Rejected or deferred **[I]**:
- **Slice-level priority.** It adds complexity for little gain once temporal layers and FEC exist. Keyframe slices can still get extra FEC.
- **Salsify-style "encode twice, pick one".** Salsify drives the codec frame by frame from the transport's capacity estimate, using a codec that can save and restore its state **[V]**. That needs encoder state save/restore, which MPP doesn't offer. Keep its principle, a per-frame size budget, which MPP rate control can approximate.
- **Spatial SVC.** MPP has no spatial layers. Dual encode covers the use case.

## Proposed design

### 1. Priority model: every media unit carries a tag

The encoder adapter stamps each frame, and the bond sender inherits the tag for all its packets:

```
MediaTag {
  stream:     audio | video | data
  importance: 0..7        // 7 = must arrive
  deadline:   capture_ts + latency_budget
  layer:      tl0 | tl1 | tl2 | intra | ltr
  depends_on: frame id (for "useless if parent lost")
  cost_class: lifeboat_ok | normal | scavenger
}
```

Default importance mapping, which the policy can override:

| Unit | Importance | Protection |
|---|---|---|
| Control, markers | 7 | Reliable class |
| Audio | 7 | Duplicated on 2 links, Opus FEC |
| IDR / LTR-marked frame | 6 | FEC ×2, retransmit while deadline allows |
| TL0 P | 5 | FEC per measured loss, retransmit |
| TL1 | 3 | FEC light, no duplicate |
| TL2 | 1 | No FEC, first dropped |
| Telemetry | 4 (reliable) | Ordered, coalescible: the latest GPS fix supersedes older ones in the live view; the outbox still keeps them all |
| Recording segments | 0 (background) | Scavenger, gated by budget |

The scheduler rule **[I]**: under pressure, drop by ascending importance. Never send a packet whose `depends_on` frame is already lost. That last rule saves bandwidth for free.

### 2. Policy format

The policy is defined in `packages/domain` (Effect Schema), stored as JSON and edited in the UI. The agent compiles it into controller parameters and sends them over the Unix socket. The Rust side stays pure and receives only numbers.

```toml
name = "Économie de data"
base = "balanced"

[intent]                 # weights 0..1, normalized
picture  = 0.3
latency  = 0.2
data     = 0.9
battery  = 0.4

[floors]                 # hard constraints, never violated before the outage scene
audio_kbps    = 32
min_fps       = 10
min_height    = 480
max_latency_ms = 4000

[ladder]
sacrifice_order = ["fps", "resolution", "bitrate"]   # what to give up first
rungs = ["1080p30@6M", "1080p30@4M", "720p30@2.5M", "720p15@1.2M", "480p15@600k", "slideshow", "audio_map"]

[protect]
audio_duplicate = true
keyframe_fec    = "high"

[links]
"sim-local"   = { role = "primary",  budget_gb_day = "auto", price_per_gb = 2.0 }
"sim-roaming" = { role = "lifeboat", budget_gb_day = 0.2,    price_per_gb = 9.0 }
"wifi"        = { role = "primary",  metered = false }

[recording]
quality = "high"          # independent of live
upload  = { on = ["wifi", "cheap_sim_night"], backfill_outages = true, highlights_first = true }

[privacy]
gps_delay_min = 15
hide_within_m = { campsite = 500 }
```

Presets ship as documents; the user picks one, and tweaks inherit via `base`:

| Preset | Gist |
|---|---|
| **Cinéma** | Picture 1.0. Keeps 1080p and resolution, drops fps second. Latency up to 6 s, deep FEC and retransmit. Data spent freely. |
| **Talk show** | Audio sacred, double FEC. Latency 1.5–2 s. Drops fps first (15 is fine for a talking head). Centre ROI. Smart P when static. |
| **Action** (riding) | Keeps fps and drops resolution first. Short latency. Motion-aware ladder. |
| **Économie de data** | 720p max. The budget pacer is strict. Lifeboat SIMs carry audio and TL0 only. Recording uploads on Wi-Fi only. |
| **Batterie** | 720p30, one modem active, extra links woken only below the floor. Record encoder at lower bitrate. Screen and LEDs off. |
| **Silence radio** | Recording only: no live, telemetry only, everything uploads later. Useful at borders or with expensive roaming. |

There is one global toggle, "Événement" (spend multiplier ×3 for the next hour), for a moment worth burning data on **[I]**.

### 3. Cross-layer control loop (four timescales)

```
           ┌──────── policy (user) ─────────┐
           ▼                                │
 ┌─ hours/day: budget pacer ─┐  ceiling bps, link roles, upload windows
 │ GB spent vs allowance     │──────────────┐
 └───────────────────────────┘              ▼
 ┌─ ~1 s: ladder controller ─┐  rung (res, fps, target bps), FEC level, latency budget
 │ capacity est., QoE score  │──────────────┐
 └───────────────────────────┘              ▼
 ┌─ per frame 33 ms: frame budget ─┐  next-frame byte budget, TL drop,
 │ queue delay, in-flight bytes    │  USE_LTR / IDR request, PSKIP on starvation
 └─────────────────────────────────┘        ▼
 ┌─ per packet: scheduler ─┐  link choice by arrival time × cost, duplicate, FEC, drop by importance
 └─────────────────────────┘
```

- **Per packet** (transport engine, covered by another researcher): it uses `importance`, `deadline` and `cost_class`.
- **Per frame.** The transport publishes `capacity_bps` and `queued_bytes`. The encoder adapter sets the next frame's budget as `capacity × frame_interval − queue_excess` (Salsify's principle **[V]**, applied through the MPP rate control `bps` and QP bounds **[I]**). If the budget is below the TL2 frame size, skip TL2 at the source rather than encoding and dropping it. On a loss report, send `USE_LTR`. Request an IDR only if no LTR has been confirmed.
- **Per second.** The ladder controller moves one rung at a time with hysteresis: down fast, up slow, with a probe before each upgrade. The objective is to maximize the QoE score (see below) subject to the floors, the data ceiling and the battery ceiling, weighted by `intent`. A change of resolution goes through a planned IDR, while a change of fps only toggles temporal layers.
- **Per hour.** The pacer computes `allowance_today = remaining_quota / days_to_renewal`, smoothed so that an unused morning can fund an evening live. The resulting data ceiling, in bits per second, feeds the ladder. Background upload runs only when (link unmetered) OR (price < threshold AND hour in the night window AND battery/charging allows). Order of the upload queue: outage backfill proxy → highlight segments → chronological archive.

Orders of magnitude for the UI **[I, arithmetic]**: 1 Mbit/s ≈ 450 MB/h, 4 Mbit/s ≈ 1.8 GB/h, and a 12 Mbit/s H.265 record ≈ 5.4 GB/h, so about 30 GB per 6-hour riding day. The full archive realistically goes over Wi-Fi or on SSDs. Cellular uploads the proxy and the highlights.

### 4. Recording format

- Closed-GOP fMP4/CMAF segments of 2–4 s, each named by `session/capture_ts/hash`. A manifest is journaled on disk. Uploads are resumable at segment granularity, and the server acknowledges each segment by hash **[I]**.
- Two renditions are produced at record time: **archive** (high) and **proxy** (for example 720p at 1.5 Mbit/s). The proxy can come from a third, cheap MPP session or from a later re-encode on the box at night (check the power cost).
- Disk is the safety net: never delete a segment before the server has acknowledged it, and alert when the disk passes 80 %.

### 5. Metadata and server features enabled by the protocol

- A **shared media clock**: every telemetry sample (GPS, speed, cadence, heart rate over BLE, battery, link stats) carries a `capture_ts` on the same clock as the frames. The server renders overlays and charts in sync, including in replays. Telemetry goes in the reliable class, not in SEI, so it survives video loss **[I]**. The privacy policy applies on the server before display (delay, geofence).
- **Markers**: highlight, "I'm going into a tunnel", "start of the climb". They power clips, chapters and viewer bets.
- **Server DVR**: keep the last N minutes of the received live stream. Instant replay, rewind for viewers, and "seamless switch to replay" during an outage: play the last highlight or the backfilled segments while waiting.
- **Health side channel**: the box sends its intent ("degrading on purpose: Économie de data") so the dashboard and the chat can show "low data mode" instead of "bad stream" **[I]**.

### 6. What the UI exposes

- **Home:** one row of preset buttons, a "Live / Record only" switch, the "Événement" button, and a big highlight button (also mapped to a physical handlebar button).
- **Meter strip:** current rung (for example "720p25"), latency, today's data and money per SIM with a projection ("at this pace: 1.4 GB / 2 GB today"), battery hours left, and upload backlog ("3 h 20 of recording waiting, next upload: hotel Wi-Fi").
- **Advanced** (collapsed): four intent sliders, the floors, the sacrifice-order drag list, link roles and prices, upload windows and privacy. Each change shows "what you'll lose" in one line ("in weak coverage you'll get 15 fps instead of 720p").
- **For IRL streamers:** the same document, shareable as a file ("my Tokyo night preset").

## What to measure

| Signal | Where | Use |
|---|---|---|
| Encoder PSNR (from `KEY_ENC_SSE`), average QP, real bps | Box, per frame **[V API]** | Encoding distortion, rung quality |
| Frames delivered / late / lost per layer, decode errors | Server | Transport effectiveness |
| Freeze count and duration, using the WebRTC rule: gap ≥ max(3 × avg frame duration, avg + 150 ms) **[V]** | Server, after decode | Main smoothness KPI |
| Audio concealment ms, glitches | Server | "Audio sacred" KPI |
| Glass-to-glass latency p50/p99 | Box clock + server | Latency target |
| VMAF of the live stream vs the recording, same timestamps | Server, offline | **Free ground truth**: the full-quality recording arrives later, so we can compute real full-reference VMAF for every minute of live and calibrate the online score **[I]** |
| Online QoE score = f(bits per pixel, QP, fps, freeze ratio, resolution switches) | Server → box | Objective for the ladder controller, fitted on the VMAF pairs above |
| € and GB per QoE point, Wh per hour of live | Box | Compare presets honestly |
| Recording completeness, backlog age | Server | Goal #1 |

For benchmarks, add these metrics to the M1b table, compared against the SRTLA/SRT baseline: mean QoE score, freeze ratio, audio glitch rate and the cost of an hour of live.

## How the others do it (on this angle only)

| | Encoder ↔ transport | Media priority | User trade-offs | Recording |
|---|---|---|---|---|
| LiveU LRT | ABR tells the encoder to lower its bitrate, plus dynamic FEC, ACK/resend and reordering **[S]** | Not public | Priority between network types (cellular, Wi-Fi, LAN, satellite) **[S]** | — |
| TVU (ISX) | Adapted to the links **[S]** | — | — | Dual encoding with "store and forward" and progressive download of a local recording **[S]** |
| Dejero Smart Blending | Blends links packet by packet in real time **[S]** | — | Data-usage reports, no documented cost caps found **[S]** | — |
| SRT | None (stats only); drops packets that are too late, optional row/column FEC filter | None | Latency setting only | — |
| WebRTC | Tight: congestion control drives the encoder; SVC, frame-ack/LTR draft **[V]** | Temporal layers, RED/FEC audio | None for end users | — |
| BELABOX / NOALBS | belacoder adapts bitrate from SRT stats; NOALBS switches OBS scenes on thresholds (low bitrate default 500 kbps, RTT 1000 ms) **[V]** | None | Min/max bitrate, latency | — |

Our gap to fill **[I]**: nobody exposes a **user-owned policy** that covers data money, battery and privacy. Nobody combines temporal-layer drop, LTR recovery and audio-first protection over bonded cellular. Nobody uses the delayed full-quality recording as a feature: backfill, highlights and VMAF ground truth.

## Open questions

1. Do TSVC, LTR use/mark, intra refresh and ROI actually work on VEPU580 in H.265 at 1080p30, and at what bitrate cost? (Bench in M3/M4, using `mpi_enc_test`.)
2. Does the Osmo Action record to its own SD card while in UVC webcam mode? If yes, that is a free third rendition and a backup.
3. The DJI Mic arrives through the camera (UAC): what sample rate, and is there latency or drift against the video?
4. How many cores does the server re-encode need, and does YouTube's HEVC enhanced-RTMP ingest (in beta at launch in 2023 **[V]**) make an H.265 output viable, or do we send H.264?
5. Should the policy use weighted intents or a strict lexicographic order? Weights are flexible but less predictable, while an order is easy to explain. A proposed compromise: floors and the sacrifice order are strict, and weights only choose within what's allowed.
6. Power: Wh per hour for one versus two modems and one versus two encode sessions (M3 measurement). This decides what "Batterie" actually does.
7. Privacy defaults for an IRL product: is a delayed and geofenced GPS opt-out or opt-in?
8. Who sets the price per GB? The user types it, or we keep a crowd-sourced table per country and operator.

## Sources

- Rockchip MPP headers (develop): https://github.com/rockchip-linux/mpp/blob/develop/inc/rk_venc_ref.h, https://github.com/rockchip-linux/mpp/blob/develop/inc/rk_venc_cmd.h, https://github.com/rockchip-linux/mpp/blob/develop/inc/mpp_meta.h, https://github.com/rockchip-linux/mpp/blob/develop/inc/rk_mpi_cmd.h, https://github.com/rockchip-linux/mpp/blob/develop/utils/mpi_enc_utils.c
- gstreamer-rockchip MPP encoder (mirror): https://github.com/JeffyCN/mirrors/tree/gstreamer-rockchip/gst/rockchipmpp
- Opus 1.5 / DRED: https://opus-codec.org/demo/opus-1.5 , https://bloggeek.me/webrtcglossary/dred/
- Salsify (NSDI 2018): https://www.usenix.org/conference/nsdi18/presentation/fouladi , https://snr.stanford.edu/salsify
- IETF Video Frame Acknowledgement draft: https://www.ietf.org/archive/id/draft-ietf-avtcore-frame-acknowledgement-01.html
- WebRTC freeze stats: https://developer.mozilla.org/en-US/docs/Web/API/RTCInboundRtpStreamStats/freezeCount , https://developer.mozilla.org/en-US/docs/Web/API/RTCInboundRtpStreamStats/totalFreezesDuration , https://bugs.webkit.org/show_bug.cgi?id=212938
- LiveU LRT: https://www.liveu.tv/lrt , https://www.liveu.tv/resources/blog/not-all-bonding-is-the-same , https://videoguys.com/blogs/news-and-sales/what-is-cellular-bonding-and-how-does-liveu-lrt-actually-work
- TVU: https://www.tvunetworks.com/how-to-use-progressive-download-mode-tvu-one/ , https://www.tvunetworks.com/wp-content/uploads/2019/07/TVU-One-v3_-Datasheet.pdf
- Dejero: https://dejero.com/technology/smart-blending-technology
- NOALBS: https://github.com/NOALBS/nginx-obs-automatic-low-bitrate-switching
- HEVC ingest: https://blog.wmspanel.com/2023/05/enhanced-rtmp-hevc-av1.html , https://veovera.org/docs/news/introducing-enhanced-rtmp , https://developers.google.com/youtube/v3/live/guides/ingestion-protocol-comparison
