---
id: 48
title: Split Project Index (Vault vs Code)
status: proposed
priority: high
labels:
  - refactor
---

# ISS-0048 :: Split Project Index (Vault vs Code)

## Intent

`VaultIndex` currently mixes two concerns:

- vault indexing (markdown files, headings, links, inbound maps, vault checks)
- code indexing (workspace package maps, language workspace caches, resolved code imports)

Split these into explicit components so each has a clear boundary and API.

## Scope

- Introduce a `ProjectIndex` wrapper (or equivalent) with separate fields:
  - `vault`: markdown vault facts + link graph
  - `code`: code import/workspace facts
- Move code-related fields out of `VaultIndex` (workspace packages, per-language workspace caches, `code_imports`).
- Update CLI/LSP call sites:
  - commands that only need vault data should not pull in code indexing
  - commands that need code imports should depend on the code index explicitly
- Keep public APIs stable during transition via re-exports or compatibility constructors where needed.

## Why

- Clarifies ownership and reduces `god struct` pressure.
- Makes incremental refactors safer: vault-only changes don’t touch code indexing.
- Enables `ProjectContext` (iss-0047) + indexing to compose cleanly.

## Depends on

- iss-0047 (ProjectContext + shared discovery/path/ignore semantics)
