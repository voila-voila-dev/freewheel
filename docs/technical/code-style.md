# Code style

Code is read far more than it is written. Everything here aims at reading it once and understanding it.

## One thing per file

- A file holds **one** concept: one type, one function family, one benchmark, one scenario.
  Its name says which: `departure.rs`, `data-plan.ts`, `desert_day.rs`.
- A folder groups a concept and its parts; its `mod.rs` / `index` only declares and re-exports.
- Tests live next to what they test, one file per topic:
  Rust integration tests in `tests/<topic>.rs`, TypeScript in `<name>.test.ts`.

## Explicit over clever

- Names say what things are, in full words: `queued_bytes`, `renewalDay`, `capacity_share`.
  No abbreviations except units (`_ms`, `_bytes`, `_bps`, `_mw`).
- Units are always in the name. A number without a unit is a bug waiting to happen.
- Every public item has a doc comment saying what it is for, not how it works.
- No magic numbers: a constant with a name and a comment saying where the value comes from.

## Layers (TypeScript, as in tries.care)

- `packages/domain`: pure model, no I/O, by bounded context:
  `aggregates/`, `value-objects/`, `errors/`, `use-cases/`, `services/`.
- `apps/*`: adapters and composition roots only.
- Imports through `#` aliases, never long relative paths.

## Rust

- No third-party crates unless `decisions.md` says why.
- `unsafe` is forbidden, except in a dedicated, documented crate (`alloc-counter`, future FFI crates),
  with a `SAFETY:` comment on every block.
- No `unwrap`, `expect` or `panic!` outside tests: errors are values.
- Hot paths allocate nothing per packet (checked by `crates/benchmarks/tests/zero_allocations.rs`).

## Checks

`bun run check`, `bun run check-types`, `bun run test`,
`cargo clippy --workspace --all-targets -- -D warnings`, `bun run bench`.
