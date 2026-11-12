# Benchmarks

Everything that runs on the box is measured, because the box runs on a battery. See also
[`bench/README.md`](../../bench/README.md) and the research behind the thresholds:
[`research/05-embedded-performance.md`](research/05-embedded-performance.md).

## What is measured

- **Binary size** of the hot-path binary, per crate and per hot path (`nm`, `size-report` profile).
- **Instructions per packet**, counted by Cachegrind: deterministic, so CI can gate on it.
- **Allocations per packet**: must be zero (`crates/benchmarks/tests/zero_allocations.rs`).
- **Agent size**: the ARM64 executable shipped to the box.

## Adding a benchmark

1. Add `crates/benchmarks/src/benchmarks/<name>.rs` exporting a `BENCHMARK`.
2. Register it in `crates/benchmarks/src/benchmarks/mod.rs`.
3. It is picked up by the zero-allocation test and by `bun run bench` automatically.

## Gates

| Metric | Fails when it grows by more than |
|---|---|
| Instructions per packet | 2 % |
| Binary and crate size | 2 %, with 16 KiB tolerated |
| Agent size | 3 % |
| Allocations per packet | anything above 0 |

Coming next (from the research): peak memory, syscalls per packet, watts on the board.
