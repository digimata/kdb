---
id: 32
title: Consolidate stubs and orphans into check flags
status: proposed
priority: medium
labels:
  - cleanup
  - cli
---

# ISS-0032 :: Consolidate stubs and orphans into check flags

## Intent

`kdb orphans` and `kdb stubs` are standalone commands that should be flags on `kdb check`. They're diagnostics, not distinct operations.

## CLI

```
kdb check                # broken links only (current default)
kdb check --orphans      # include orphan files (already exists)
kdb check --stubs        # include empty stubs (new)
kdb check --all          # all diagnostics
```

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `--stubs` and `--all` flags to `Check` subcommand; deprecate/remove `Orphans` and `Stubs` subcommands |
| `src/cmd.rs` | Move stubs logic into `check()`, remove `orphans()` and `stubs()` entrypoints |
