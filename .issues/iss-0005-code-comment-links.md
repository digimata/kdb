---
id: 5
title: Code Comment Links Into KDB Docs
status: proposed
priority: high
labels:
  - roadmap
---

# 0005 :: Code Comment Links Into KDB Docs

Allow code comments to include kdb links (for example `/// see [[docs/overview.md#runtime]]`) and make them navigable.

- LSP supports goto-definition, hover, completion, and diagnostics for kdb links in comments.
- CLI can validate comment links with `kdb check --code-links`.
- Link resolution behavior matches markdown link behavior.
