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
