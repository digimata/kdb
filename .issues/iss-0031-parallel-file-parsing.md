---
id: 31
title: Parallel tree-sitter file parsing during index build
status: proposed
priority: high
labels:
  - perf
  - index
---

# ISS-0031 :: Parallel tree-sitter file parsing

## Intent

Parallelize tree-sitter parsing across files during index build. For a 10k-file monorepo at ~3ms per parse, going from 1 core to 8 cores cuts cold build from ~30s to ~4s.

## How it works

- Use `ignore`'s parallel walker (iss-0030) or rayon to distribute files across a thread pool
- Each thread gets its own tree-sitter parser instance (parsers are not `Send` — allocate per-thread)
- Collect per-file facts (symbols, imports, references) into a concurrent structure
- After parallel parse phase, build cross-reference maps single-threaded (cheap hashmap insertions)

## Considerations

- Tree-sitter `Parser` is not thread-safe — must be created per-thread, not shared
- Per-file facts are independent — no coordination needed during parsing
- Cross-reference map building must happen after all files are parsed (needs the full import map)
- For small projects (<100 files), parallelism overhead may exceed benefit — consider a threshold

## Dependencies

- **iss-0030** (`ignore` crate): provides the parallel file walker
- **iss-0028** (code symbol refs): the index build pipeline that gets parallelized

## Changes

| File | Change |
|---|---|
| `Cargo.toml` | Add `rayon` dependency (if not using `ignore`'s parallel walker directly) |
| `src/index/mod.rs` | Parallelize file parsing in `VaultIndex::build()` |
| `src/index/code.rs` | Ensure per-file extraction is stateless / thread-safe |
