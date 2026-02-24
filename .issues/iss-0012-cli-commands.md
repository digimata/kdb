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
- [x] `kdb check`
- [x] `kdb outline`
- [x] `kdb lsp`

## In progress

- [ ] `kdb fmt` → [iss-0014](iss-0014-code-file-indexes.md)
- [ ] `kdb codemap` → [iss-0016](iss-0016-codemap.md)

## Phase 1 — markdown queries

- [x] `kdb symbols` → [iss-0018](iss-0018-symbols-command.md)
- [ ] `kdb refs` → [iss-0019](iss-0019-refs-command.md)
- [ ] `kdb deps` → [iss-0020](iss-0020-deps-command.md)
- [ ] `kdb graph` → [iss-0021](iss-0021-graph-command.md)

## Phase 2 — code symbol queries

- [ ] `kdb symbols <file.rs>` — extends iss-0018 with code file support

## Phase 3 — code dependency graph

- [ ] `kdb deps <file.rs>` — import → file resolution per language
- [ ] `kdb refs <module>` — reverse import lookup
- [ ] `kdb graph` for code files

## Retired

- `kdb orphans` → folded into `kdb check --orphans`
- `kdb stubs` → TBD, may become a `check` flag
- `kdb tree` → replaced by `kdb codemap`
