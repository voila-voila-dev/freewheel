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
