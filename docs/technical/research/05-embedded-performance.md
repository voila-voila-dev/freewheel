# 05 — Embedded performance and measuring everything

Research angle: how big, how hungry and how hot every piece of freewheel is, and how we keep it that way.
Date: 2026-11-10. Tags used below: **[V]** verified from a cited source, **[I]** inference or estimate to confirm on the box, **[R]** read from this repo.

## Context

- Box: Radxa ROCK 5T, RK3588 (4x Cortex-A76 up to 2.4 GHz with 512 KB L2 each, 4x Cortex-A55 up to 1.8 GHz with 128 KB L2 each) **[V]**, 8 GB LPDDR5, MPP hardware encoder, powered by ~100 Wh USB-C power banks, 10 h/day live, 10–15 W total target.
- Radxa ships the ROCK 5T on the Rockchip vendor kernel 6.1; mainline device-tree support was posted by Collabora in May 2025 and applied **[V]**. Everything below works on 6.1 (UDP GSO since 4.18 **[V]**, UDP GRO since 5.0 **[I]**, SO_TXTIME since 4.19 **[I]**, io_uring SEND_ZC since 6.0 **[I]**).
- Rust workspace, zero third-party crates, `unsafe` forbidden except in dedicated FFI crates, release profile `lto="fat"`, `codegen-units=1`, `opt-level=3`, `strip=true` **[R]**. Crates today: `bonding-protocol` (140 lines), `bitrate-control` (43), `link-emulator` (131) **[R]**: there is no hot path yet, so this is the right time to put measurements in place before code exists.
- Agent: Bun 1.4.4 + Effect 4, `bun build --compile --target=bun-linux-arm64`; the current executable is 109,300,728 bytes for an `index.ts` that only logs a line **[R]**. So ~100% of that size is the Bun runtime, not our code.
- CI: GitHub Actions `ubuntu-24.04` x64 only; `docs/technical/testing.md` plans QEMU user mode for ARM64 **[R]**. Roadmap M1 step 6 and M3 already ask for "bench against upstream" and "real watts" **[R]**: this document turns those into a framework.

## Problem

1. A battery box has a fixed energy budget: 100 Wh / 10 h = **10 W average per battery per day**. Every watt we waste is ~10 % of the day's streaming. Software overhead is cheap to fix early and very expensive to find later.
2. "Fast on my laptop" means nothing on an A55 at 408 MHz idle clocks (the ROCK 5T idles at 408 MHz **[V]**). Wall-clock benchmarks in a shared CI VM are noise; QEMU user mode is not timing-representative at all.
3. Without a number in CI, regressions creep in one innocent `Vec::new()` or `format!` at a time.
4. The 109 MB agent raises real questions (RAM, flash, cellular update cost) that need data, not opinions.

## Needs

- One number per question, measured the same way every time, stored in the repo, compared on every PR.
- Deterministic metrics in CI (instruction counts, allocation counts, byte sizes); physical metrics (watts, °C, real µs) on the board, on a schedule.
- No new runtime dependencies: measurement tools are dev tools (valgrind, perf, binutils, a USB-C meter), and our own glue stays small (Rust test helpers + Bun scripts).
- Respect the `unsafe` rule: anything that needs raw syscalls or a global allocator hook lives in a dedicated crate.

## Measurement framework

### What we measure (the metric catalog)

| # | Metric | Unit | Where | Tool | CI or board |
|---|---|---|---|---|---|
| M1 | Binary size per executable, per section (`.text`, `.rodata`, `.data`, `.bss`) | bytes | Rust bins, agent | `size -A`, `llvm-size` | CI |
| M2 | Size per symbol / per crate (top 50) | bytes | Rust bins | `nm --size-sort -S -C` on a non-stripped build, grouped by crate path prefix by a Bun script | CI |
| M3 | Instructions per packet (send path, receive path, FEC encode/decode, AEAD seal/open, scheduler decision) | instr/pkt | Rust | Cachegrind (`valgrind --tool=cachegrind --cache-sim=no`) on a fixed workload, divided by packet count | CI |
| M4 | Heap allocations on the hot path | count/pkt (target **0**) | Rust | counting `#[global_allocator]` in a test-only crate | CI |
| M5 | Peak and steady memory | KB | every process | `/proc/<pid>/status` `VmHWM`, `VmRSS`; `/proc/<pid>/smaps_rollup` (Rss, Pss, Private_Dirty) | CI + board |
| M6 | Syscalls per packet | syscalls/pkt | sender, receiver | `strace -c -f` (CI, coarse), `perf trace -s` or bpftrace `tracepoint:raw_syscalls:sys_enter` (board) | CI + board |
| M7 | CPU cost per Mbps | % of one core per Mbps, per cluster | sender, encoder, agent | `/proc/<pid>/stat` utime+stime over 60 s, `perf stat -e cycles,instructions,cache-misses` | board |
| M8 | Latency added per stage | µs p50/p99/p99.9 | capture→encode→packetize→schedule→send; recv→reorder→output | `CLOCK_MONOTONIC` timestamps carried in a debug header, histogram in-process | CI (relative) + board |
| M9 | Power | W total, W per Mbps, W per active link | whole box, then per rail | USB-C PD logger at the power-bank output; INA226 shunt per rail later | board |
| M10 | Thermal | °C per zone, throttle events | SoC | `/sys/class/thermal/thermal_zone*/temp`, `cpufreq` `scaling_cur_freq` | board |
| M11 | Startup time | ms to "ready" | agent, sender, receiver | timestamp at `exec` vs first "ready" log; `systemd-analyze blame` | CI + board |
| M12 | Flash writes | MB/hour | whole system | `/sys/block/mmcblk*/stat` or `/proc/diskstats` field "sectors written" × 512, sampled hourly | board |
| M13 | Update payload | MB per release | agent, Rust bins, OS image | full size, `zstd -19` size, `zstd --patch-from=<previous>` delta size | CI |

### How (per metric family)

**Size (M1, M2, M13).** `strip=true` in the release profile removes the symbol table, so add a `[profile.size-report]` that `inherits = "release"` with `strip = false` and `debug = false`. Run `nm --size-sort -S -C --radix=d` on it and fold symbol names by crate prefix (`bonding_protocol::`, `core::`, `std::`) in a ~60-line Bun script; this gives 90 % of what `cargo-bloat` gives with zero installed tool beyond binutils. `cargo-bloat` and `twiggy` stay optional local tools for deep dives; Google's `bloaty` is the best one if we want section × symbol views. Report the **stripped** release size as the gate number (that is what ships), and the per-symbol view as the explanation.
Also measure `panic = "abort"` and `opt-level = "s"` variants once: on a tiny hot path, `-O3` vs `-Os` often changes size by 20–40 % for <5 % speed **[I]**; decide with numbers.

**Instructions (M3).** Valgrind Cachegrind counts executed instructions (`Ir`) deterministically: the same binary on the same input gives the same count to within a few instructions, which is why the Iai / Gungraun family (Gungraun is the renamed iai-callgrind) uses it for CI benchmarking in noisy VMs **[V]**. We do not need Gungraun itself (third-party, and our rule is no crates): a bench binary per hot path runs N=100,000 packets through the pure code, and a Bun script runs `valgrind --tool=cachegrind --cache-sim=no --cachegrind-out-file=...`, parses the `summary:` line and divides by N. Use client requests or a "warm-up then measure" split by running the binary twice (N and 2N) and taking the difference, which cancels startup cost without any instrumentation API. Caveat **[V]**: instruction count ignores cache misses and branch mispredictions; it is a regression detector, not a speed measurement. That is exactly what CI needs. Run it on the ARM64 runner so counts are AArch64 instructions (x64 counts are still useful as a trend, not as an absolute).

**Allocations (M4).** A test-only crate `alloc-counter` (allowed `unsafe`, like an FFI crate, documented in `decisions.md`) wraps `std::alloc::System` and increments an `AtomicU64` on `alloc`/`realloc`. Tests assert `allocs_after - allocs_before == 0` around 10,000 steady-state packets. This is a hard gate: zero means zero. DHAT/heaptrack (`valgrind --tool=dhat`, `heaptrack`) are for investigating, not gating.

**Memory (M5).** Read `VmHWM` (peak RSS) and `smaps_rollup` at the end of each e2e scenario; for Bun read it from outside the process. Track **Private_Dirty** separately: shared file-backed pages (the mmapped 109 MB executable) are reclaimable, dirty anonymous pages are not.

**CPU and syscalls (M6, M7).** On the board: `perf stat -e task-clock,cycles,instructions,context-switches,cpu-migrations -p <pid> -- sleep 60` at fixed bitrates (2, 6, 12, 20 Mbps) and fixed link counts (1, 2, 4). Express the result as **% of one A55 core per Mbps** and **instructions per packet including kernel** (`instructions` without `:u`). `perf trace -s` or `bpftrace -e 'tracepoint:raw_syscalls:sys_enter /pid == X/ { @[probe] = count(); }'` gives syscalls per packet. Check the vendor kernel config first: `CONFIG_PERF_EVENTS`, `CONFIG_BPF_SYSCALL`, `CONFIG_FTRACE` must be on **[I]**; if not, our own OS image (M6 of roadmap) turns them on in a debug variant.

**Latency per stage (M8).** Each stage stamps `CLOCK_MONOTONIC_RAW` into a fixed-size log ring (no allocation); a debug build dumps fixed-bucket histograms (log2 µs buckets, 32 counters per stage). In CI over the `link-emulator`, compare only relative numbers; on the board, absolute.

**Power (M9).** Three levels, cheapest first:
1. **USB-C inline logger** between power bank and box: an FNIRSI FNB58 class meter logs V/A/W over USB HID to Linux with open-source loggers (`baryluk/fnirsi-usb-power-data-logger`, `OpenFNB58`) **[V]**. Log at 10 Hz to CSV from the dev laptop. This is the ground truth for the whole system and is enough for M3 of the roadmap.
2. **INA226 shunt per rail** (box 5 V/12 V input, modem rail, camera rail) on the box's I²C, read through the mainline `ina2xx` hwmon driver: `power1_input` in µW, `curr1_input` in mA, `shunt_resistor` settable at run time, `update_interval` for averaging on INA226 **[V]**. This lets the agent itself log watts per subsystem in the field, i.e. "W per link" becomes a telemetry field, not a lab experiment. Good candidate for the custom hardware milestone.
3. **SoC-internal**: the RK3588 PMIC exposes regulators (voltages) but, as far as we found, no per-rail current telemetry **[I]**; do not count on it. Use the energy model indirectly: cpufreq residency (`/sys/devices/system/cpu/cpufreq/policy*/stats/time_in_state`) × known per-OPP power once measured with method 1.
Measurement protocol: fixed scene (static test pattern vs. outdoor clip), 10 min warm-up, 30 min measure, ambient °C recorded, report mean W and W per Mbps delivered. Always subtract an **idle baseline** measured the same day (board idle is ~2.4–3.1 W on the sibling ROCK 5B **[V]**, 3.9 W or more on the ROCK 5T with desktop image **[V, ambiguous source]**).

**Thermal and DVFS (M10).** Log every thermal zone and every cpufreq policy at 1 Hz with the power log. RK3588 has separate frequency domains for the A55 cluster and the two A76 pairs **[I: commonly reported, check `ls /sys/devices/system/cpu/cpufreq/`]**. A throttling event in a sealed bag in summer is a stream outage: count them.

**Flash (M12).** eMMC wear is a writes problem, not a size problem. Sample `sectors written` hourly during the 10 h bench run; target below 50 MB/h for the whole system excluding the local recording **[I]**. Journald in `Storage=volatile`, telemetry outbox batched (one fsync per N seconds, not per point), recordings on a separate medium (NVMe or SD) so the system eMMC only sees updates.

### Benchmark scenarios (shared by all metrics)

S1 "idle armed": agent + sender up, no stream. S2 "nominal": 6 Mbps H.265, 2 links. S3 "max": 20 Mbps, 4 links, 20 % FEC. S4 "storm": 4 links with 10 % loss, 200 ms jitter, one link flapping every 30 s (from `link-emulator` seeds and recorded traces). Every metric is reported per scenario so numbers are comparable over time.

## CI gates

- **Where.** GitHub `ubuntu-24.04-arm` runners are now available to private repositories too (since 2026-01-29; 2 vCPU / 8 GB in private repos, 4 vCPU in public ones; counted against free minutes) **[V]**. Run size, instruction and allocation jobs natively there. Drop QEMU user mode for anything performance-related: it emulates, so its timing is meaningless and Valgrind under QEMU is fragile **[I]**. Keep QEMU only for "does it run" smoke tests if wanted.
- **Hardware counters.** `perf stat` inside a hosted VM may not expose PMU counters **[I]**; do not depend on them in hosted CI. Use Cachegrind there; use `perf` on the board.
- **Later: self-hosted runner on a second ROCK 5T** (labels `self-hosted, rk3588`), nightly not per-PR, with the USB-C logger attached: real µs, watts, °C per scenario, results pushed as an artifact and appended to a history file. Never run it on public-PR events (security).
- **Baselines in the repo.** `bench/baseline.json` holds every gated number. A PR that changes a number must update the file; the diff is the review. No external SaaS needed (Bencher supports Gungraun if we ever want graphs **[V]**).
- **Thresholds** (start strict, loosen only with a written reason in `decisions.md`):

| Metric | Gate |
|---|---|
| Allocations per steady-state packet | must be 0 (hard) |
| Instructions per packet, per hot path | fail if > +2 % vs baseline |
| Stripped size of each Rust executable | fail if > +2 % or > +16 KiB |
| Agent executable size | fail if > +3 % |
| Agent delta update (`zstd --patch-from`) | warn if > 5 MB |
| Peak RSS per process in e2e scenarios | fail if > +10 % |
| Syscalls per packet (strace, S2) | fail if > +5 % |
| Agent startup to ready (CI) | warn if > +20 % (wall time is noisy) |

Improvements below baseline by more than 5 % also fail softly ("update the baseline"), so the baseline stays honest.

## Hot-path optimization (Linux ARM64, UDP multipath)

Rough load: 20 Mbps at ~1,316-byte payloads ≈ 1,900 pkt/s, ~2,300 with 20 % FEC, plus ACK/NAK traffic **[I]**. This is small for the CPU; the cost is dominated by **per-packet kernel work** (syscall entry, socket lookup, skb alloc, driver, USB modem stack), not by our code. So optimize in this order:

1. **Batch syscalls.** `sendmmsg`/`recvmmsg` move from one syscall per packet to one per batch. Cloudflare found syscall count dropped sharply but throughput rose less, because the stack cost remains **[V]**.
2. **UDP GSO (`UDP_SEGMENT`, Linux ≥ 4.18) and UDP GRO (`UDP_GRO`).** GSO hands the kernel one large buffer that it cuts into equal-size datagrams late in the stack; it beats sendmmsg because the savings accrue across the stack, not just at the syscall **[V]**, and combines with sendmmsg **[V]**. The UDP GRO commit reported rx going from 769,065 to 24,877 calls/s for more throughput **[V]**. Fit for us: each link is its own socket bound to one interface with one destination, so a burst of same-size packets scheduled to one link is exactly the GSO case. Constraint: the scheduler must emit **per-link bursts of equal-size segments** (last one may be shorter). Whether the modem USB NIC (cdc_ether/qmi_wwan/mhi) does hardware segmentation does not matter much; software GSO still saves stack passes **[I]**. GSO also delays packets until a burst is formed: cap a burst by time (e.g. 1–2 ms) so it never adds latency beyond pacing.
3. **Pacing: `SO_TXTIME` + `fq` qdisc** lets us give each packet a departure time and lets the kernel pace it, instead of user-space sleep loops (which cost wakeups and power). Fits the "arrival-time scheduler" of M1b. Measure that `fq` is available in the vendor kernel **[I]**.
4. **io_uring vs epoll.** For ~2,000 pkt/s across 4 sockets, `poll`/`epoll` + `recvmmsg`/`sendmmsg` + GSO is enough and is plain std + a tiny FFI **[I]**. io_uring's advantage (fewer syscalls with SQPOLL, multishot recv, provided buffers) pays at much higher rates, and SQPOLL burns a core, which is the opposite of what a battery wants. Zero-copy send (`IORING_OP_SEND_ZC`/`MSG_ZEROCOPY`) does not pay for MTU-sized packets: kernel developers quote ~16 KB as the break-even for MSG_ZEROCOPY and benchmarks show 1,200-byte zero-copy sends slower than copies **[V]**. Decision: epoll/poll first, keep io_uring as a measured experiment behind the same port. This answers the open question in `decisions.md` ("std + poll vs mio vs tokio"): std + poll + own FFI, measured.
5. **Zero allocation, fixed pools.** Preallocate a slab of packet buffers (e.g. 4,096 × 1,500 B = 6 MB, sized from the max retransmit window) indexed by `u16`/`u32` handles; the ARQ window, FEC groups and send queues hold handles, never `Vec<u8>`. Rings with power-of-two capacity, no `HashMap` on the hot path (sequence numbers are dense: index `seq & mask`). Gate M4 enforces it.
6. **Cache-friendly layout.** Struct-of-arrays for per-packet metadata (seq, send time, link id, state) so a scan of the window touches 1–2 cache lines per 16 packets; keep hot state per link under 64 B; avoid false sharing between the rx and tx threads (pad to 64 B; A76 and A55 use 64-byte lines **[I]**).
7. **FEC with NEON.** Reed–Solomon over GF(2^8) is dominated by table-lookup multiplies; the standard trick is split-nibble tables with `TBL` (`vqtbl1q_u8`), 16 bytes per instruction, which is what ISA-L and similar libraries do on AArch64 **[I]**. XOR-only codes (simple parity across a group) are nearly free. `core::arch::aarch64` NEON intrinsics are `unsafe`, so they live in a small `fec-simd` crate with a scalar reference implementation and a property test `simd == scalar`. Measure instr/byte both ways before deciding it is worth the unsafe.
8. **AEAD cost.** The RK3588 cores report `aes pmull sha1 sha2` in `/proc/cpuinfo`, on the A76 and on the A55 **[V, community captures, not Rockchip docs]**: the ARMv8 Cryptography Extensions are present. So AES-128-GCM (AESE/AESMC + PMULL for GHASH) is the cheap choice on this chip, likely around 1 cycle/byte on A76 **[I]**; ChaCha20-Poly1305 on NEON would be several cycles/byte **[I]**. At 20 Mbps = 2.5 MB/s, even 5 cycles/byte is 12.5 M cycles/s, under 1 % of one A55: crypto is not the bottleneck, but pick AES-GCM when the audited crate uses the CE path (check it does runtime detection for `aes`/`pmull` on aarch64), and measure M3 for seal/open. Add a boot check that logs the cpuinfo flags into telemetry.
9. **Threads, pinning, governor.** Hypothesis to test **[I]**: the whole transport fits on one A55 at a moderate OPP, and A55 work costs less energy per instruction than A76 work, while the encoder pipeline lives on the MPP block. Experiment matrix: transport pinned to A55 vs A76 vs unpinned × governor `schedutil` vs `ondemand` vs fixed OPP, measure W (M9) and p99 latency (M8) at S2/S3. Pin with `sched_setaffinity` (FFI crate) or systemd `CPUAffinity=`. Keep the A76 clusters off when not needed so they can sit in deep idle; one thread for rx+tx per process with poll timeouts aligned to pacing slots (fewer wakeups = less power; count wakeups with `perf stat -e context-switches` and `powertop`-style residency).
10. **Wakeups as a metric.** On a battery, an idle box waking 1,000 times per second is a bug. Measure wakeups/s in S1 for every process; target < 50/s total for our code **[I]**.

FFI note: std exposes none of `sendmmsg`, `UDP_SEGMENT`, `SO_TXTIME`, `SO_BINDTODEVICE`, `sched_setaffinity`. One crate (`sys` or `net-sys`) declares the handful of `extern "C"` items from glibc, with a safe wrapper and tests; that keeps the zero-crate rule and the unsafe rule.

## Bun agent

What 109 MB means, metric by metric:

- **Flash: no.** Even two A/B copies (≈220 MB) are noise on eMMC/NVMe. **[I]**
- **RAM: much less than 109 MB.** The executable is mapped from file; only touched pages become resident, and they are clean, reclaimable pages. What matters is JavaScriptCore heap + JIT + private dirty pages. Measure M5 (VmHWM, Private_Dirty) of the current hello-world build on the box as the floor before writing features **[I: expect tens of MB]**.
- **Updates over cellular: yes, this is the one that matters.** 109 MB × a roaming SIM is real money and minutes on a weak link.
- **Startup: medium.** Matters for restart-after-crash time, not for daily boot.

Levers, in order of expected gain (Bun 1.4 docs) **[V]**:
1. **`--trim`** (experimental): removes the package manager, test runner, type checker, and with `bun compile` the Bun/Node APIs the code never calls. Bun's own table: the `cli` feature alone is 47.4 MB on Linux x64, `jit` 14.1 MB, `intl` 10.5 MB, `lint` 12.2 MB, `bundler` 6.0 MB, `interpreter` 3.7 MB; an example trims an executable to 33.4 MB. `bun check --aot --explain=trim` lists what is kept and why.
2. **`--trim-locales en,fr`**: drops most of the 10.5 MB of Intl data.
3. **`--disallow-code-generation-from-strings`** removes the interpreter for `eval`/`new Function` (3.7 MB) and unblocks trimming; check Effect does not need `new Function`.
4. **`--minify`**: megabytes for large apps; ours is small, so little.
5. **`--bytecode`** (+ `--bytecode-order` profile-guided): 1.5–4x faster startup, in one app cold start 1.0 s → 0.53 s and startup memory −40 MB; bytecode is 2–8x larger than source and version-locked to Bun.
6. **`bun compile` (= `--compile --aot`)**: ahead-of-time machine code, "faster startup and lower memory", experimental.
Plan: CI builds a matrix of these flags once, records M1/M5/M11 on the arm64 runner and on the box, and we pick with numbers. Expected landing zone ~30–45 MB **[I]**.

**Delta updates.** The Bun runtime part of the executable is identical between two releases with the same Bun version; only the embedded bundle changes. `zstd --patch-from=<installed> <new>` (or bsdiff) should therefore produce deltas of roughly the bundle size **[I: measure, gate M13]**. Pin the Bun version and upgrade it deliberately, rarely: a Bun upgrade is the one full-size download. For the A/B scheme of roadmap M6, ship deltas against the active slot with a full image as fallback; verify the signature on the reconstructed file, not the patch.

**When something moves to Rust.** Bun orchestrates; it does not touch media or packets. Move code to Rust when any of these is true:
- it runs per packet or per frame, or above ~10 Hz;
- it has a deadline under ~50 ms where a GC pause would show;
- it costs more than ~1 % of one A55 core steadily (M7) or more than ~10 MB of private dirty memory;
- it must survive the agent crashing (the stream must not stop when the UI stack restarts).
Telemetry at 1 Hz, D-Bus control, the HTTP API and the tunnel supervisor stay in Bun. The Unix-socket JSON contract (decisions) is fine at control-plane rates; measure its cost at the telemetry rate once (M7), and do not let link stats per packet cross it.

## Performance budget

For the whole box during S2 (6 Mbps, 2 links). Numbers marked **[I]** are targets to validate in roadmap M3, not measurements.

| Component | CPU (avg) | RAM (private) | Flash (installed) | Power | Notes |
|---|---|---|---|---|---|
| Board base (SoC idle, RAM, regulators, Ethernet off) | — | OS ~150 MB | OS image ≤ 1 GB | 2.5–4 W | measure idle first **[V range, I target]** |
| Encoder (GStreamer + MPP H.265) | ≤ 15 % of one A76 | ≤ 150 MB | ≤ 40 MB | ≤ 1.5 W over idle | MJPEG decode may dominate **[I]** |
| Bond sender (Rust) | ≤ 5 % of one A55 at 20 Mbps | ≤ 16 MB | ≤ 2 MB | ≤ 0.2 W | 0 alloc/pkt, ≤ 4 syscalls per 10 pkts **[I]** |
| Agent (Bun) | ≤ 2 % of one A55 avg | ≤ 60 MB | ≤ 45 MB | ≤ 0.2 W | wakeups ≤ 20/s **[I]** |
| Modems (per 4G/5G link) | — | — | — | 1–3 W each | dominant term, measure per link **[I]** |
| Camera + mic (USB) | — | — | — | 1–2 W | **[I]** |
| **Total target** | **≤ 25 % of the CPU budget** | **≤ 512 MB** | **≤ 2 GB** | **≤ 10 W at S2, ≤ 15 W at S3** | 10 W = one 100 Wh bank per 10 h day |

Other budgets: transport latency added ≤ 2 ms p99 on top of pacing; agent startup to ready ≤ 1 s; sender restart to streaming ≤ 500 ms; flash writes ≤ 50 MB/h excluding recordings; SoC ≤ 85 °C with zero throttle events at 30 °C ambient; agent update delta ≤ 5 MB.

## First benchmarks to build

In this order, each a small PR:

1. **`bench/` + `bench/baseline.json` + `bun run bench`** (Bun script): runs every measurement below, prints a table, compares to baseline with the thresholds above, exits non-zero on regression. Add an `arm64` job on `ubuntu-24.04-arm` that runs it; remove the QEMU plan for perf in `docs/technical/testing.md`.
2. **Size report.** `[profile.size-report]`; script: `size -A` per executable + `nm` top-50 folded by crate; same for `apps/agent/dist/freewheel-agent` plus its `zstd -19` size. Today's agent baseline: 109,300,728 bytes **[R]**.
3. **Agent flag matrix.** Build the agent with {default, `--minify`, `--trim`, `--trim --trim-locales en,fr`, `--bytecode`, `bun compile`}; record size, startup to ready, VmHWM and Private_Dirty after 30 s. Pick the default from data and write it in `decisions.md`.
4. **`alloc-counter` crate + first zero-alloc test**: `bonding-protocol` encode/decode of every SRTLA packet type 10,000 times allocates 0 times (decode into caller buffers).
5. **Instructions per packet for `bonding-protocol`**: a `benches/` binary with N and 2N iterations under Cachegrind, `Ir/packet` for encode and decode. Same harness later for scheduler, FEC, AEAD seal/open.
6. **Syscalls per packet in e2e**: once `bonding-sender` exists, run S2 in the netns e2e job under `strace -c -f`, gate on syscalls per packet; this is the before/after proof for sendmmsg and GSO.
7. **Board power script (M3 roadmap)**: `scripts/bench-power` logs the USB-C meter at 10 Hz plus thermal zones, cpufreq policies, `/proc/<pid>/stat` for our processes and `diskstats`, into one CSV per scenario; first runs: stock BELABOX image at S1/S2/S3 as the reference to beat, then the A55-vs-A76 pinning matrix.
8. **Telemetry fields**: agent reports W (INA226 when present), SoC °C, throttle count, RSS per process and flash MB written per hour, so the field ride becomes the long-running benchmark.

## Sources

- RK3588 cpuinfo flags (aes pmull sha1 sha2 on A76 and A55): https://openbenchmarking.org/system/2210302-NE-2210305NE40/RK3588-A76-Coremark/lscpu, https://openbenchmarking.org/s/Rockchip+RK3588
- ARMv8 Crypto Extensions detection notes: https://github.imc.re/SoCXin/sbc-bench/blob/main/results/ARMv8-Crypto-Extensions.md
- RK3588 cluster caches and clocks: https://turingpi.com/?p=722245
- ROCK 5T power (idle 408 MHz, comparisons): https://www.linuxlinks.com/radxa-rock-5t-single-board-computer-running-linux-power-consumption/, https://www.linuxlinks.com/radxa-cubie-a7a-single-board-computer-running-linux-power-consumption/
- ROCK 5B idle 2.39 W / 3.1 W with fan: https://forum.radxa.com/t/rock-5b-debug-party-invitation/10483/94?u=tkaiser
- ROCK 5T kernel 6.1 and mainline DT: https://docs.radxa.com/en/software-support-matrix/rockchip, https://patchew.org/linux/20250520-add-rock5t-v2-0-1f1971850a20@collabora.com, https://cnx-software.com/2024/12/21/rockchip-rk3588-mainline-linux-support-current-status-and-future-work-for-2025
- Bun executables, bytecode, size: https://bun.com/docs/compiler/executables, https://bun.com/docs/compiler/bytecode, https://bun.com/docs/compiler/size
- GitHub arm64 runners: https://github.blog/changelog/2026-01-29-arm64-standard-runners-are-now-available-in-private-repositories, https://github.blog/changelog/2025-08-07-arm64-hosted-runners-for-public-repositories-are-now-generally-available/
- Gungraun / iai-callgrind: https://docs.rs/crate/gungraun/, https://gungraun.github.io/gungraun/latest/html/best_practices.html, https://conjure-cp.github.io/conjure-oxide-reports/main/book/developers_guide/coding_resources/benchmarking-guide.html
- UDP GSO/GRO and sendmmsg: https://blog.cloudflare.com/accelerating-udp-packet-transmission-for-quic/, https://webcf.waybackmachine.org/web/20200809183245/http://vger.kernel.org/lpc_net2018_talks/willemdebruijn-lpc2018-udpgso-paper-DRAFT-1.pdf, https://android-kvm.googlesource.com/linux/+/bec1f6f697362c5bc635dacd7ac8499d0a10a4e7, https://android-kvm.googlesource.com/linux/+/e20cf8d3f1f763ad28a9cb3b41305b8a8a42653e
- Zero-copy send thresholds: https://lists.openwall.net/netdev/2024/04/15/340, https://man.archlinux.org/man/extra/liburing/io_uring_prep_send_zc.3.en, https://patchew.org/linux/cover.1640029579.git.asml.silence@gmail.com/
- INA2xx hwmon driver: https://android-kvm.googlesource.com/linux/+/refs/heads/master/Documentation/hwmon/ina2xx.rst
- USB-C meter loggers: https://github.com/baryluk/fnirsi-usb-power-data-logger, https://github.com/yeckel/OpenFNB58
