---
id: 52
title: "Remove persistent disk cache"
status: done
priority: high
labels:
  - refactor
---

# ISS-0052 :: Remove persistent disk cache

## Rationale

With targeted usage scanning (iss-0046), `refs -s` on kubernetes (16k files) runs in 2.8s cold vs 0.93s cached. The ~2s savings doesn't justify the complexity of the disk cache:

- Cache invalidation (mtime/size, seahash, manifest keys)
- Staleness detection + 30-day GC
- Atomic writes via tempfile
- Graceful degradation for corrupt/missing cache
- `--fresh` flag threading through every command
- `kdb index` command for manual rebuilds
- `IncrementalBuildResult` intermediate type
- bincode + seahash + tempfile dependencies

## What to remove

1. `src/index/cache.rs` — delete entirely
2. `src/index/mod.rs` — remove `build_cached*` / `build_from_cached*` methods; collapse to direct `build()` / `build_with_symbol_refs()` / `build_for_target()` calls
3. `src/cmd.rs` — remove `--fresh` flag from all commands, remove `kdb index` subcommand, simplify `CmdContext` (drop `fresh` field)
4. `src/main.rs` — remove `Index` subcommand variant
5. `Cargo.toml` — remove `bincode`, `seahash`, `tempfile` deps (check if tempfile is used elsewhere)
6. `.kdb/index.bin` — no longer generated; `kdb init` no longer calls index build
7. `indicatif` — check if still needed after removing progress bar from index

## Keep

- The targeted scanning path (`build_for_target`) — that's the real perf win
- `WorkspaceCaches` / import resolution — still computed fresh each time, just not persisted

## Verification

1. `cargo build` — zero errors
2. `cargo test` — all tests pass
3. `cargo clippy` — zero warnings
4. Benchmark: `kdb refs -s` on kubernetes, tokio, poetry — confirm cold times are acceptable
5. `kdb --help` — no `index` subcommand, no `--fresh` flag

## Benchmark results (v0.16.0)

| Repo | Files | v0.10.2 (no cache) | v0.12.0 (cached) | v0.16.0 cold | v0.16.0 warm |
|---|---|---|---|---|---|
| mio (Rust) | ~80 | 0.05s | 0.03s | 0.06s | 0.01s |
| poetry (Python) | ~470 | 0.26s | 0.12s | 0.22s | 0.07s |
| tokio (Rust) | ~790 | 0.32s | 0.17s | 0.28s | 0.08s |
| airstore (Go) | ~350 | 0.16s | 0.16s | 0.07s | 0.02s |
| kubernetes (Go) | ~12k | 9.0s | 7.2s | 1.13s | 0.91s |

Warm runs (OS page cache) beat the old disk cache across the board. Kubernetes: 7.2s → 0.91s.
