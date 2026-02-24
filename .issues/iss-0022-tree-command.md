---
id: 22
title: kdb tree Command
status: proposed
priority: high
labels:
  - feat
  - cli
---

# 0022 :: kdb tree Command

## Intent

Give agents and humans a quick structural overview of the project. Like `tree` but kdb-aware — respects ignore patterns, skips build artifacts, and only shows what matters.

## Usage

```
kdb tree [path]          # print filtered directory tree
kdb tree --depth <n>     # limit depth
kdb tree --json          # machine-readable output
```

- `path` defaults to project root.
- Respects `.kdb/config.toml` ignore patterns (same as `kdb fmt` and `kdb check`).
- Always skips `.kdb/`, `.git/`, `target/`, `node_modules/`, etc.

## v0 — Filtered tree

Verbatim tree-style output (connectors, indentation) but filtered through kdb's ignore system. No annotations. This is the baseline that agents can use immediately for orientation.

```
.
├── Cargo.toml
├── CODEMAP.md
├── README.md
├── src
│   ├── cmd.rs
│   ├── config.rs
│   ├── fmt
│   │   ├── mod.rs
│   │   └── preamble.rs
│   ├── index
│   │   ├── mod.rs
│   │   └── refs.rs
│   ...
└── tests
    ├── cli.rs
    └── index.rs
```

## v1 — Annotated tree (future)

Add optional per-file metadata to help agents decide where to dive:

```
kdb tree --annotate
```

```
src/
  cmd.rs              9 symbols   "CLI command entrypoints"
  config.rs           2 symbols   "Config parsing"
  index/
    mod.rs           12 symbols   "Markdown parser and vault indexer"
    refs.rs           5 symbols   "Inbound reference collection"
notes/
  overview.md         5 headings  3 inbound  7 outbound
  setup.md            2 headings  1 inbound  0 outbound
```

Annotations could include: symbol/heading count, inbound/outbound link count, first line of module doc comment.

## Implementation

v0 is straightforward:

1. Resolve root, load ignore patterns (same as other commands)
2. Walk the directory tree, filter through ignore patterns
3. Render with tree-style connectors (`├──`, `└──`, `│`)
4. Print to stdout

Most of the walker logic already exists in `fmt::format_workspace`. Extract the shared directory walking into a common utility or reuse the same pattern.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Tree` subcommand |
| `src/cmd.rs` | Add `cmd::tree()` entrypoint |
| `src/tree.rs` (new) | Tree rendering — walk + format with connectors |
| `tests/cli.rs` | Integration tests |
