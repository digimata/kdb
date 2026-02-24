---
id: 21
title: kdb graph Command
status: proposed
priority: medium
labels:
  - feat
---

# ISS-0021 :: kdb graph Command

## Intent

Output the full dependency graph for a vault or subtree. Useful for visualization, cluster detection, and understanding the overall structure of a knowledge base.

## Usage

```
kdb graph [path]
kdb graph [path] --json
kdb graph [path] --cluster
```

## Output

Dot (default):
```
$ kdb graph docs/
digraph {
  "tutorial.md" -> "hooks.md"
  "tutorial.md" -> "components.md"
  "hooks.md" -> "state.md"
}
```

JSON:
```
$ kdb graph --json docs/
[
  {"from": "tutorial.md", "to": "hooks.md"},
  {"from": "tutorial.md", "to": "components.md"},
  {"from": "hooks.md", "to": "state.md"}
]
```

Cluster:
```
$ kdb graph --cluster docs/
Cluster 1: tutorial.md, hooks.md, state.md
Cluster 2: api.md, endpoints.md, auth.md
Orphans: changelog.md
```

## Flags

- `--dot` — Graphviz dot format (default)
- `--json` — structured edge list
- `--cluster` — detect clusters of related files

## Implementation

- Build `VaultIndex`, iterate all files and their links
- If `path` is a subdirectory, filter to files under that subtree
- Dot output: emit `digraph` with one edge per resolved link
- Cluster detection: connected components or community detection on the link graph

## Scope

Markdown-only for now. Code dependency graph (phase 3) will extend this.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Graph` subcommand |
| `src/cmd.rs` | Add `cmd::graph()` entrypoint |
