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
included). Median of the runs. `--gate <receiver>-<sender>` checks a candidate against the
baseline. Method in `labs/srtla-capture/README.md` ("Transport bench").

Baseline of 2026-11-17, one run on a laptop (OrbStack, ARM64): indicative until CI's three runs.

| Scenario | Sender → receiver | Frames lost or damaged | Longest gap | Delivered | Latency p50 / p99 |
|---|---|---|---|---|---|
| link-drop | upstream → upstream | 1 | 33 ms | 3343 kbit/s | 2060 / 2081 ms |
| | ours (compatibility) | 0 | 0 ms | 3344 kbit/s | 2061 / 2078 ms |
| | irlserver → upstream | 2 | 33 ms | 3342 kbit/s | 2066 / 2098 ms |
| burst-loss | upstream → upstream | 2 | 33 ms | 3356 kbit/s | 2060 / 2095 ms |
| | ours | 0 | 0 ms | 3358 kbit/s | 2062 / 2084 ms |
| | irlserver | 0 | 0 ms | 3358 kbit/s | 2075 / 2229 ms |
| varying-bandwidth | upstream → upstream | 0 | 0 ms | 3339 kbit/s | 2064 / 2084 ms |
| | ours | 1 | 33 ms | 3338 kbit/s | 2062 / 2079 ms |
| | irlserver | 0 | 0 ms | 3339 kbit/s | 2073 / 2257 ms |
| all-links-drop | upstream → upstream | 211 | 6930 ms | 2686 kbit/s | 2061 / 2074 ms |
| | ours | 210 | 6930 ms | 2685 kbit/s | 2061 / 2086 ms |
| | irlserver | 213 | 6930 ms | 2676 kbit/s | 2063 / 2082 ms |
| real-ride | upstream → upstream | 70 | 825 ms | 3287 kbit/s | 2126 / 2157 ms |
| | ours | 79 | 792 ms | 3282 kbit/s | 2110 / 2296 ms |
| | irlserver | 79 | 462 ms | 3286 kbit/s | 2137 / 2158 ms |
| trace-city | upstream → upstream | 2 | 66 ms | 3333 kbit/s | 2119 / 2135 ms |
| | ours | 1 | 33 ms | 3335 kbit/s | 2118 / 2138 ms |
| | irlserver | 2 | 33 ms | 3335 kbit/s | 2118 / 2201 ms |
| trace-drive | upstream → upstream | 1766 of 2711 | 6105 ms | 1869 kbit/s | 2083 / 2119 ms |
| | ours | 1760 | 2046 ms | 1874 kbit/s | 2081 / 2122 ms |
| | irlserver | 1903 | 5082 ms | 1718 kbit/s | 2095 / 2376 ms |

Reading:
- **Emulated impairments no longer separate anyone.** Loss, bursts, a dead link, varying
  capacity: SRT's 2 s buffer hides them for all three (0–2 frames). A 6 s total outage costs
  everyone the same 7 s: nothing to win there without the recording.
- **`trace-drive` is where M1b is won.** Three recorded LTE uplinks, 5.3 Mbit/s together on
  average for a 3.4 Mbit/s stream, and two thirds of the video is lost, by all three senders.
  Upstream's logic drops the Verizon link after a dip and does not take it back for 40 s while
  it carries 1–3 Mbit/s again, and floods the 0.9 Mbit/s ATT link instead (characterisation
  test `trace_drive`). irlserver does no better.
- **Latency**: 2.06–2.13 s median, SRT's 2 s buffer plus the network. A transport that recovers
  losses itself can shorten that buffer: the p50 / p99 columns say by how much.
