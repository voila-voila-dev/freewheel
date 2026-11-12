# Architecture

```
Osmo Action (UVC MJPEG 1080p30) ──USB──┐
DJI Mic (UAC via camera) ──────────────┤
                                       ▼
                         ┌──────────── board (RK3588) ────────────┐
                         │ encoder: GStreamer + MPP H.265, rate    │
                         │ bond sender: SRTLA over N links         │
                         │ agent (Bun + Effect): net, modems,      │
                         │   GPS outbox, tunnel, control API       │
                         └───────┬───────────────┬─────────────────┘
                     4G/5G SIM(s), Wi-Fi         │ telemetry (store-and-forward)
                                 ▼               ▼
                         ┌──────────── own server (VPS) ───────────┐
                         │ bond receiver → SRT listener             │
                         │ telemetry ingest + storage               │
                         │ dashboard, remote control over tunnel    │
                         │ outage scenes, replays → YouTube/Twitch  │
                         └─────────────────────────────────────────┘
```

## Upstream reference (pinned)

| Repo | Fork | Commit |
|---|---|---|
| srtla | `voila-voila-dev/srtla` | `37862da3d0c13b46956efd3f88877053293d97d6` |
| belacoder | `voila-voila-dev/belacoder` | `6588b6bddc3e0aed2eefe9506423e6f6057370c3` |
| belaUI | `voila-voila-dev/belaUI` | `665930824abd6e74638f4b4726b84fc9a88cfb60` |

Upstream notes `srtla_rec` is "unsupported … not suitable for production" and points to their paid cloud: our receiver is a first-class component here.
