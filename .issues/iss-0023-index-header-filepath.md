---
id: 23
title: Use file path as index block header
status: done
priority: medium
labels:
  - enhancement
  - fmt
---

# ISS-0023 :: Use file path as index block header

## Intent

Replace the generic `## Index` header in managed code index blocks with the file's relative path. More useful information in the same space — an agent or human reading the block immediately knows which file they're looking at.

## Before

```rust
// ## Index
//
// fn lsp()         L33
// fn init()        L38
```

## After

```rust
// src/cmd.rs
//
// fn lsp()         L33
// fn init()        L38
```

## Notes

- Path is relative to project root, same as everywhere else in kdb
- Path changes if file is moved, but `kdb fmt` rewrites the block on every run so it's always current
- Detection: the managed block marker changes from looking for `## Index` to looking for a line matching the file's own relative path (or any path-like pattern as fallback)
- This also makes codemap assembly cleaner — the per-file header in `kdb codemap` output already uses the file path, so the index block and codemap output are now consistent

## Changes

| File | Change |
|---|---|
| `src/fmt/mod.rs` | `render_block()` — emit file path instead of `## Index`; `find_managed_block()` — detect by file path pattern |
