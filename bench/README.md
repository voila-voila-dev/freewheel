# Benchmarks

`bun run bench` measures, compares with `baseline.<platform>.json`, and fails on a regression.
`bun run bench --update` records the current numbers as the new baseline (commit it).

One baseline per platform (`darwin-arm64`, `linux-arm64`…): sizes and instruction counts differ
between them. The CI gate uses `linux-arm64`, the same platform as the box; produce it in an
OrbStack Linux VM (`orb`), which is ARM64 on Apple Silicon.

| Metric | How | Gate |
|---|---|---|
| `size.binary.hot-paths.bytes` | file size, `size-report` profile | +2 % or +16 KiB |
| `size.crate.<crate>.bytes` | `nm --print-size`, summed per crate | +2 % or +16 KiB |
| `instructions.<benchmark>.per_packet` | Cachegrind, two runs, difference / extra packets | +2 % |
| `size.agent.bytes` | `bun build --compile` ARM64 executable | +3 % |
| allocations per packet | `crates/benchmarks/tests/zero_allocations.rs` | must be 0 (test) |

Instruction counts need Valgrind (Linux): CI runs them on a native ARM64 runner.
`results.json` is the last run, not committed.

## SRTLA bonding, end to end

`labs/srtla-capture/bench.sh` then `bench-srtla` (CI job `srtla-bench`, native ARM64 runner):
each sender against upstream's receiver, each receiver behind upstream's sender, 3 runs. Best run
for CPU, memory and delivery (a busy host only makes them worse), median for recovery. The gate
fails if ours is behind upstream; method in `labs/srtla-capture/README.md` ("Bench").

Results of 2026-11-12 (CI run 43253012709, `d999ef2`):

| Sender → receiver | 50 Mbit/s delivered | CPU sender / receiver, 50 Mbit/s 20 s | CPU sender / receiver, 3 Mbit/s 30 s | Peak memory sender / receiver | 12 Mbit/s over 2 × 8 Mbit/s | Dead link: damaged frames | Restored link share 15–25 s after |
|---|---|---|---|---|---|---|---|
| upstream → upstream | 100 % | 1090 / 1280 ms | 200 / 190 ms | 1552 / 1556 kB | 100 % | 0 | 12 % |
| **ours** → upstream | 100 % | 1210 / 1280 ms | 190 / 190 ms | 1800 / 1548 kB | 100 % | 0 | 14 % |
| upstream → **ours** | 100 % | 1210 / 1330 ms | 190 / 200 ms | 1556 / 1736 kB | 100 % | 0 | 11 % |
| irlserver → upstream | 100 % | 2330 / 690 ms | 470 / 160 ms | 8464 / 1544 kB | 100 % | 0 | 47 % |

Reading:
- **On par.** Same delivery and recovery as upstream, which compatibility mode reproduces by
  design. CPU within 11 % (our sender at 50 Mbit/s, the closest to the gate: the first thing to
  look at if it flakes), memory within 0.25 MB.
- **irlserver** costs twice the sender CPU and five times the memory, but halves the receiver's
  CPU and gives a restored link its traffic back four times faster (47 % against 12 %): it does
  not reproduce upstream's slow ramp. That is the bar for M1b, not upstream.
- Neither throughput nor aggregation separates anyone: 50 Mbit/s on clean links and 12 Mbit/s
  over two 8 Mbit/s links are delivered in full by all. Harder cases belong to M1b's bench, on
  recorded traces.

## Transport (M1b)

`labs/srtla-capture/bench-transport.sh` then `bench-transport` (CI job `transport-bench`): what
M1b is judged on, per scenario. Frames lost or damaged and longest gap (frame by frame),
delivered bitrate, encoder-to-server latency over the intact frames (SRT's 2 s buffer
included), restored link share on `link-drop`. Median of the runs. `--gate <receiver>-<sender>`
checks a candidate against the baseline (and irlserver for the restored link). Method in `labs/srtla-capture/README.md` ("Transport bench").

Baseline of 2026-11-17 (CI run 44085568015, `e649d57`, median of 3 runs on the ARM64 runner):

| Scenario | Sender → receiver | Frames lost or damaged | Longest gap | Delivered | Latency p50 / p99 |
|---|---|---|---|---|---|
| link-drop | upstream → upstream | 0 | 0 ms | 3343 kbit/s | 2058 / 2064 ms |
| | ours (compatibility) | 0 | 0 ms | 3343 kbit/s | 2058 / 2064 ms |
| | irlserver → upstream | 0 | 0 ms | 3343 kbit/s | 2060 / 2068 ms |
| burst-loss | upstream → upstream | 0 | 0 ms | 3357 kbit/s | 2058 / 2064 ms |
| | ours | 0 | 0 ms | 3357 kbit/s | 2057 / 2064 ms |
| | irlserver | 0 | 0 ms | 3357 kbit/s | 2062 / 2069 ms |
| varying-bandwidth | upstream → upstream | 0 | 0 ms | 3338 kbit/s | 2059 / 2068 ms |
| | ours | 0 | 0 ms | 3338 kbit/s | 2059 / 2068 ms |
| | irlserver | 0 | 0 ms | 3338 kbit/s | 2065 / 2074 ms |
| all-links-drop | upstream → upstream | 209 | 6897 ms | 2687 kbit/s | 2057 / 2064 ms |
| | ours | 209 | 6897 ms | 2686 kbit/s | 2057 / 2064 ms |
| | irlserver | 210 | 6930 ms | 2683 kbit/s | 2062 / 2069 ms |
| real-ride | upstream → upstream | 52 | 924 ms | 3296 kbit/s | 2110 / 2140 ms |
| | ours | 55 | 594 ms | 3295 kbit/s | 2120 / 2135 ms |
| | irlserver | 108 | 660 ms | 3276 kbit/s | 2118 / 2144 ms |
| trace-city | upstream → upstream | 0 | 0 ms | 3336 kbit/s | 2123 / 2133 ms |
| | ours | 0 | 0 ms | 3337 kbit/s | 2123 / 2133 ms |
| | irlserver | 2 | 33 ms | 3336 kbit/s | 2090 / 2099 ms |
| trace-drive | upstream → upstream | 1768 of ~2700 | 3432 ms | 1808 kbit/s | 2078 / 2146 ms |
| | ours | 1726 | 3927 ms | 1838 kbit/s | 2077 / 2115 ms |
| | irlserver | 1956 | 8844 ms | 1706 kbit/s | 2093 / 2394 ms |

Reading:
- **Emulated impairments no longer separate anyone.** Loss, bursts, a dead link, varying
  capacity: SRT's 2 s buffer hides them for all three (0 frames). A 6 s total outage costs
  everyone the same 7 s: nothing to win there without the recording.
- **`trace-drive` is where M1b is won.** Three recorded LTE uplinks, 5.3 Mbit/s together on
  average for a 3.4 Mbit/s stream, and two thirds of the video is lost, by all three senders.
  Upstream's logic drops the Verizon link after a dip and does not take it back for 40 s while
  it carries 1–3 Mbit/s again, and floods the 0.9 Mbit/s ATT link instead (characterisation
  test `trace_drive`). irlserver does no better.
- **Latency**: 2.06–2.13 s median, SRT's 2 s buffer plus the network. A transport that recovers
  losses itself can shorten that buffer: the p50 / p99 columns say by how much.
