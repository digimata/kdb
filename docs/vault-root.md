---
path: kdb/docs/vault-root.md
outline: |
  • Vault Root (.kdb/)               L11
    ◦ Why require a vault root?      L24
    ◦ How root discovery works       L35
    ◦ Monorepo usage                 L47
    ◦ What lives in .kdb/            L59
---

# Vault Root (`.kdb/`)

Every kdb command requires a `.kdb/` directory somewhere in the ancestor path of the target file. This directory is created by [`kdb init`](kdb://kdb/README.md#quickstart) and marks the **vault root** — the boundary of the project that kdb indexes and operates on.

```
my-project/          <-- vault root
  .kdb/
    config.toml
  src/
  docs/
  README.md
```

## Why require a vault root?

kdb builds a **vault-wide index** of every file in the project — markdown headings, code symbols, import graphs, and cross-file references. This index is what makes commands like `refs`, `deps`, and `symbols` work with precision instead of being glorified grep.

Without a vault root:

- **No project boundary.** kdb wouldn't know where to stop walking. Should `kdb refs` scan your home directory? The entire filesystem?
- **No cross-file resolution.** Import resolution (`use crate::api::auth`) requires knowing the full set of files in the project and their relationships. A single-file tool can't resolve `@kernl-sdk/protocol` to `packages/protocol/src/index.ts` — that requires the workspace context.
- **No disambiguation.** Two files can define `fn handle()`. Without the import graph, a reference to `handle` is ambiguous. The vault index resolves this by tracing imports through to their definitions.
- **No caching.** The persistent index (`.kdb/index.bin`) lets kdb skip re-parsing unchanged files. Without a stable root directory, there's nowhere to store or invalidate the cache.

## How root discovery works

When you run any kdb command, it walks up from the target file (or current directory) looking for a `.kdb/` directory. The first one it finds becomes the vault root. All file paths in the index are stored relative to this root.

```
~/projects/              <-- .kdb/ here = vault root
  kdb/src/cmd.rs         → indexed as kdb/src/cmd.rs
  kernl/packages/...     → indexed as kernl/packages/...
```

This means a single `.kdb/` at a monorepo root indexes everything — Rust crates, TypeScript packages, Python modules, Go packages — all in one vault with cross-project visibility.

## Monorepo usage

For monorepos, place `.kdb/` at the top level. kdb's index is language-indifferent — all languages are stored homogeneously in the same index. Language boundaries emerge naturally from import resolution: Rust `use` statements resolve to `.rs` files, TypeScript `import` statements resolve to `.ts` files, and so on.

```
~/projects/              <-- kdb init here
  .kdb/
  app-rust/src/...
  app-ts/packages/...
  shared-python/...
```

## What lives in `.kdb/`

| File | Purpose |
|---|---|
| `config.toml` | Project configuration (name, ignore patterns) |
| `index.bin` | Persistent index cache (auto-generated, safe to delete) |
