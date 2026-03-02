---
id: 1
title: KDB Workspaces
status: proposed
priority: medium
labels:
  - roadmap
path: kdb/.issues/iss-0001-kdb-workspaces.md
outline: |
  • ISS-0001 :: KDB Workspaces      L13
---

# ISS-0001 :: KDB Workspaces

Support kdb workspaces with monorepo-style structure for multiple knowledge bases in one repo.

- Workspace root and package boundaries are defined.
- CLI and LSP resolve links correctly across workspace packages.
- `kdb check` and index behavior are deterministic in multi-package repos.
