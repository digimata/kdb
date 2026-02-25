---
id: 12
title: CLI Commands
status: in_progress
priority: high
labels:
  - tracking
---

# ISS-0012 :: CLI Commands

## Intent

Tracking issue for the full kdb CLI surface area.

## Shipped

- [x] `kdb init`
- [x] `kdb check` (with `--orphans`)
- [x] `kdb tree`
- [x] `kdb symbols` (markdown + code, `-s` selection, `--json`, `--public`)
- [x] `kdb refs` (markdown inbound refs, `--json`, `--count`)
- [x] `kdb refs -s` (code symbol references, `-c` context, `--json`, `--count`)
- [x] `kdb deps` (markdown + code, `--json`)
- [x] `kdb fmt` (code index headers, LSP formatter chain)
- [x] `kdb lsp`

## Not yet implemented

- [ ] `kdb graph` — dependency graph output (stubbed, not implemented) → [iss-0021](iss-0021-graph-command.md)
- [ ] `kdb codemap` — unified agent-readable codebase map → [iss-0016](iss-0016-codemap.md)

## Retired

- `kdb orphans` → folded into `kdb check --orphans`
- `kdb stubs` → folded into `kdb check --orphans`
- `kdb outline` → removed, use `kdb symbols` instead
