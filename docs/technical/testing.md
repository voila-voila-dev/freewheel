# Testing without hardware

Every physical part has a stand-in that runs on a laptop and in CI.

| Real thing | Stand-in | Where |
|---|---|---|
| Cellular links (loss, jitter, outages) | `link-emulator` seeded emulator; Linux network namespaces + `tc netem` for end-to-end runs | `crates/link-emulator`, CI `e2e` job |
| Upstream behaviour | Original C `srtla_send` / `srtla_rec` built from pinned forks, run in `labs/srtla-capture/` over 35 network scenarios (namespaces + `tc netem`), one characterisation test each in `crates/packet-capture/tests/lab/`; `RECEIVER=freewheel` swaps in our receiver, `SENDER=freewheel` our sender, and the same tests judge them | CI `srtla-lab`, `interop` and `interop-sender` jobs |
| Camera (UVC MJPEG) | GStreamer `videotestsrc` → `jpegenc`; `v4l2loopback` device on Linux | encoder tests |
| Hardware encoder (MPP) | Software `x265enc` in CI, MPP only in on-board tests | encoder tests |
| Modems, SIM, signal | Fake `ModemManager` / `NetworkManager` adapters behind ports | `apps/agent` tests |
| GPS | Replay of recorded NMEA / GPX tracks | `apps/agent` tests |
| Power bank, heat | Scripted battery and temperature curves | `apps/agent` tests |
| ARM64 board | Cross-build for `aarch64-unknown-linux-gnu`, run under QEMU user mode | CI `arm64` job |

## Test layers

1. **Pure unit tests**: wire format, bitrate controller, outbox. Fast, run everywhere.
2. **Property tests**: random packet streams, random link fates, invariants hold.
3. **Interop tests**: our sender ↔ upstream receiver, upstream sender ↔ our receiver.
4. **End-to-end**: full pipeline over emulated links with scripted outages.
5. **On-board**: same suites on the real board, plus MPP and real modems.
