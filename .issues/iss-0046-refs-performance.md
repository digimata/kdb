---
id: 46
title: "Profile and optimize refs -s performance"
status: proposed
priority: medium
labels:
  - perf
depends:
  - 29
---

# ISS-0046 :: Profile and optimize `refs -s` performance

## Intent

`kdb refs -s` is still slow on large repos. On kubernetes (~12k files), a single symbol lookup takes ~7.2s. Profile the command end-to-end and identify the bottleneck(s).

## Current baselines (v0.12.0)

| Repo | Files | `refs -s` | Command |
|---|---|---|---|
| mio (Rust) | ~80 | 0.03s | `kdb refs src/lib.rs -s event` |
| poetry (Python) | ~470 | 0.12s | `kdb refs src/poetry/console/application.py -s Application` |
| tokio (Rust) | ~790 | 0.17s | `kdb refs tokio/src/runtime/mod.rs -s context` |
| airstore (Go) | ~350 | 0.16s | `kdb refs cmd/tools/shim/main.go -s Config` |
| kubernetes (Go) | ~12k | 7.2s | `kdb refs pkg/api/pod/util.go -s VisitContainers` |

All repos at `~/Documents/repos/<name>`. Measured with `time` (wall clock).

## Suspected bottlenecks

1. **Usage scanning (`link_usage_refs`)** — re-reads every source file and runs tree-sitter to scan for identifier usages. This is the one step that can't be skipped by caching symbols+imports alone. Likely the dominant cost at scale.
2. **Import resolution** — `build_workspace_import_index()` walks all code files, parses, and resolves imports. Will be addressed by iss-0029 (persistent cache).
3. **Symbol extraction** — `extract_symbols()` parses every file with tree-sitter. Also addressed by iss-0029.

## Approach

1. Add `--timings` flag to print per-phase breakdown to stderr (walk, parse, resolve, usage scan, query)
2. Profile kubernetes with `--timings` to confirm where time is spent
3. Optimize the dominant phase(s)

## Possible optimizations

- **Cache usage scan results per-file** — if the scanned identifiers + their locations are cached alongside symbols/imports, usage scanning can be skipped for unchanged files
- **Scope usage scanning** — only scan files that import the target file (skip files that can't possibly reference the symbol)
- **Lazy source loading** — only read source text for files that pass the import filter
