---
id: 34
title: Performance profiling and benchmarks
status: proposed
priority: high
labels:
  - perf
---

# ISS-0034 :: Performance profiling and benchmarks

## Intent

Establish a profiling and benchmarking harness so we can measure and track CLI performance on large repos. Every command should have a known performance envelope before we ship to real monorepos.

## What to measure

| Command | Target metric | Concern |
|---|---|---|
| `kdb symbols <file>` | Single file parse time | Should be <50ms regardless of repo size |
| `kdb symbols <file> -s <name>` | Single file parse + body extraction | Same |
| `kdb refs <file>` | Index build + query | Scales with repo size |
| `kdb refs -s <name>` | Index build + import resolution + query | The expensive path |
| `kdb deps <file>` | Import resolution for one file | Workspace discovery cost |
| `kdb fmt .` | Full repo walk + parse + rewrite | Scales with file count |
| `kdb check` | Full index build + link validation | Scales with file count |
| `kdb tree` | Directory walk | Should be near-instant |

## Approach

1. **Benchmark fixtures**: create or identify a large test repo (or use an existing OSS monorepo like chromium, linux, or a TS monorepo) as a benchmark target
2. **`--timings` flag**: add an optional flag that prints per-phase timing breakdown (walk, parse, resolve, query) to stderr
3. **Criterion benchmarks**: add `benches/` with criterion for hot paths (tree-sitter parse, index build, import resolution)
4. **CI regression check**: track benchmark results across commits to catch regressions

## Performance budgets (from iss-0028/0029)

| Project size | Cold index build | Warm (no changes) | Single file query |
|---|---|---|---|
| 30 files | ~100ms | ~20ms | <10ms |
| 1,000 files | ~3s | ~100ms | <10ms |
| 10,000 files | ~30s | ~500ms | <10ms |

## Changes

| File | Change |
|---|---|
| `benches/` (new) | Criterion benchmarks for parse, index build, resolution |
| `src/main.rs` | Add `--timings` global flag |
| Various | Instrument timing points with `std::time::Instant` |
