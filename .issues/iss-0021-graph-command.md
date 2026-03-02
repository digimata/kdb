---
id: 21
title: kdb graph Command
status: proposed
priority: medium
labels:
  - feat
path: kdb/.issues/iss-0021-graph-command.md
outline: |
  • ISS-0021 :: kdb graph Command             L22
    ◦ Intent                                  L24
    ◦ Usage                                   L28
    ◦ Output                                  L36
    ◦ Flags                                   L66
    ◦ Implementation                          L72
    ◦ Scope                                   L79
    ◦ Notes                                   L83
      ▪ Typed edges for agent navigation      L85
    ◦ Changes                                 L93
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

## Notes

### Typed edges for agent navigation

Consider supporting typed/labeled edges rather than bare `from → to`. This would let an agent query richer relationships — e.g. `implements <Interface>`, `imports <module>`, `extends <class>`, `tests <function>`. Even for markdown, edges could carry labels like `references`, `defines`, `see-also`.

This makes the graph useful not just for visualization but as a **queryable knowledge graph** — an agent could ask "what implements Resolver?" or "what tests build_workspace_import_index?" and get structured answers.

Might be premature for v1 but worth designing the edge model to accommodate labels from the start (e.g. an optional `kind` field on edges) so we don't have to retrofit later.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Graph` subcommand |
| `src/cmd.rs` | Add `cmd::graph()` entrypoint |
