# SRTLA capture lab

A Linux container that runs **upstream** `srtla_send` and `srtla_rec` (pinned commit) with a
real SRT stream over emulated cellular links, and captures what they put on the wire. It is how
[`docs/technical/protocol/srtla.md`](../../docs/technical/protocol/srtla.md) was written, and
the reference our own receiver and sender will be measured against.

Upstream is AGPL-3.0: this lab only *runs* it. Nothing from it is copied into freewheel.

## Topology

Three network namespaces, one veth pair per modem on each side:

```
box                          net (the cellular networks)                  server
ffmpeg → SRT caller →        b<k> ◀─ downlink netem      uplink netem ─▶ s<k>   srtla_rec :5000
srtla_send :6001 ─ m<k> ──── 10.<k>.0.1                        10.100.<k>.1 ── n<k>   → SRT listener
   10.<k>.0.2 (source routing per modem)                                       receiver 10.200.0.1
```

- `tc netem` in `net` shapes each link and direction separately: delay, rate, queue, loss
  (random or Gilbert-Elliott bursts), jitter, reordering, total cuts.
- Optional carrier NAT (iptables SNAT in `net`) to emulate 4G CGNAT port changes.
- Two captures per run: `box` (what the sender really sent, including into dead links) and
  `server` (what made it through).

## Scenarios

One file per scenario in `scenarios/`; `run-scenario --list` prints them.

| Family | Scenarios |
|---|---|
| Baseline | `steady`, `four-links`, `asymmetric-links`, `low-bitrate`, `congested`, `long-run` (10 min) |
| Link quality | `random-loss`, `burst-loss`, `jitter-reordering`, `bufferbloat`, `latency-spike`, `varying-bandwidth` |
| Outages | `link-drop` (10 s dead zone), `link-drop-long` (20 s), `link-flapping`, `modem-unplugged`, `uplink-only-drop`, `downlink-only-drop`, `all-links-drop` (6 s tunnel), `all-links-lost` (25 s) |
| Real conditions | `real-ride` (2 min, everything at once) |
| Addressing | `same-carrier`, `nat-rebinding`, `address-change`, `link-added`, `link-removed` |
| Lifecycle | `cold-start`, `receiver-restart`, `sender-restart`, `srt-server-restart` |
| Limits and abuse | `too-many-links` (9 for 8 allowed), `two-senders`, `group-limit` (200 fake groups), `hostile-packets`, `group-hijack` (sniffed group id) |

A scenario sets `LINKS` (how many modems) and defines `run()` with the building blocks of
`lib/`: `start_all`, `cut_link`, `restore_link`, `shape_link`, `unplug_modem`, `carrier_nat`,
`shared_carrier_nat`, `reload_sender_links`, `stop_receiver`, `sample_memory`, `send_hex`,
`start_eavesdropping`… Every action calls `mark`, so the analyser
can place it on the timeline.

## Running

```sh
labs/srtla-capture/run-all.sh            # every scenario, 4 at a time (~13 min)
labs/srtla-capture/run-all.sh 1 steady   # one scenario
cargo run --release -p packet-capture --bin analyze-srtla-capture -- .cache/srtla-captures           # summary table
cargo run --release -p packet-capture --bin analyze-srtla-capture -- .cache/srtla-captures link-drop # one scenario
cargo test --release -p packet-capture --test lab                                                    # characterisation tests
```

### Against our receiver

```sh
RECEIVER=freewheel labs/srtla-capture/run-all.sh                     # → .cache/srtla-captures-freewheel-receiver/
FREEWHEEL_CAPTURES_DIR=$PWD/.cache/srtla-captures-freewheel-receiver cargo test --release -p packet-capture --test lab
```

### Against our sender

```sh
SENDER=freewheel labs/srtla-capture/run-all.sh                       # → .cache/srtla-captures-freewheel-sender/
FREEWHEEL_CAPTURES_DIR=$PWD/.cache/srtla-captures-freewheel-sender cargo test --release -p packet-capture --test lab
```

`run-all.sh` builds `bonding-receiver` and `bonding-sender` for Linux in
`rust:1.94.0-slim-trixie` (the lab's Debian), mounts them at `/opt/freewheel`, and the
scenarios start ours in place of `srtla_rec` (`RECEIVER=freewheel`) or `srtla_send`
(`SENDER=freewheel`), with the same command line. One side changed at a time is an interop
test ("upstream sender → our receiver", "our sender → upstream receiver"); both set gives
Rust ↔ Rust in `.cache/srtla-captures-freewheel/` (CI job `rust-to-rust`).

Leave the machine idle while it runs: an overloaded host paces the encoders unevenly and the
measurements drift.

Runs need `docker run --privileged` (network namespaces, tc, iptables). Each run writes to
`.cache/srtla-captures/` (never committed): `<scenario>.box.pcap`, `.server.pcap`, `.marks`,
the received video `.ts`, what was sent `.sent.<k>.ts`, the frame digests `.frames`, the SRT listener statistics `.srt-stats.json` and every process log.

### End-to-end, frame by frame

The encoder also writes what it sends to `<scenario>.sent.1.ts`. At the end of a run, both it and
the received `<scenario>.ts` are hashed per compressed video frame (`ffmpeg -copyts -c copy -f
framemd5` → `.sent.1.frames`, `.received.frames`), and `FrameCheck` compares them by timestamp:
intact, damaged (arrived different or not at all), late start (before the SRT connection was up)
and cut (after the last frame that arrived), plus the longest gap.

```sh
cargo test --release -p packet-capture --test end_to_end -- --nocapture   # any captures directory, via FREEWHEEL_CAPTURES_DIR
```

`tests/end_to_end.rs` holds one limit per scenario. Unlike the characterisation tests they are
requirements: every combination must meet them, and CI checks all four.

`crates/packet-capture/tests/lab/` holds one characterisation test per scenario: what upstream
does there, measured on its captures. They are skipped when the lab has not been run, and
required in CI (`FREEWHEEL_REQUIRE_CAPTURES=1`).

`crates/packet-capture/tests/fixtures/steady-start.pcap` is the first 120 packets of a `steady`
run, committed so the basic wire facts are checked on every build, without Docker.

Long scenarios set `CAPTURE_SNAPLEN` to keep only headers (the analyser handles truncated
frames). Still missing: adaptive encoder bitrate, duplicated or corrupted packets, small MTU,
IPv6, and replays of real 4G traces recorded on the bike.

It runs anywhere Docker runs. On a Mac, OrbStack runs it as native ARM64 Linux, like the box.
