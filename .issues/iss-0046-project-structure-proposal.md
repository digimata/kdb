---
id: 46
title: Project Structure Proposal — Shared Components + Boundaries
status: proposed
priority: high
labels:
  - refactor
---

# ISS-0046 :: Project Structure Proposal — Shared Components + Boundaries

## Intent

Propose an ideal `src/` structure that makes shared logic explicit, reduces duplicated utilities (paths/ignore/discovery/text), and clarifies boundaries between:

- project context (root/config/ignores)
- indexing (markdown vault vs code imports)
- symbol extraction
- formatting
- CLI + LSP wiring

This is a design/proposal issue. Implementation should be broken into small, reviewable sub-issues/PRs.

This proposal supersedes iss-0045 (Conventions Refactor tracking). iss-0046 is the new tracking issue for the structure/conventions refactor work.

## Sub-issues (ordered)

| Order | Issue | Scope |
|---|---|---|
| 1 | [iss-0039](iss-0039-lift-codelanguage.md) | Lift `CodeLanguage` to `src/lang.rs` |
| 2 | [iss-0047](iss-0047-project-context.md) | Introduce `project/` shared modules + `ProjectContext` |
| 3 | [iss-0040](iss-0040-cmd-context.md) | Extract CLI `CmdContext` over `ProjectContext` |
| 4 | [iss-0042](iss-0042-symbol-extractor-context.md) | Extract symbol `Extractor` context struct |
| 5 | [iss-0043](iss-0043-workspace-caches.md) | Consolidate workspace caches into `WorkspaceCaches` |
| 6 | [iss-0044](iss-0044-move-tsjs-workspace-logic.md) | Move TS/JS workspace logic out of shared resolver module |
| 7 | [iss-0048](iss-0048-split-project-index.md) | Split vault indexing from code indexing (`ProjectIndex`) |
| 8 | [iss-0016](iss-0016-codemap.md) | Update `CODEMAP.md` to match the adopted structure |

## Context (what the outlines show today)

Key modules and their current responsibilities:

- `src/cmd.rs`: command entrypoints; repeats “resolve path -> find root -> load ignores -> build index” setup
- `src/index/mod.rs`: markdown vault index + link graph + also stores code import maps/workspace caches
- `src/resolve/mod.rs`: language dispatch + many shared helpers + TS/JS workspace discovery and package export resolution
- `src/discovery.rs`: ignore globset + file discovery helpers
- `src/tree.rs`: its own ignore logic + its own discovery + its own always-ignored dirs
- `src/lsp/backend.rs`: path helpers + document buffer + byte/position conversions
- `src/symbols/mod.rs`: defines `CodeLanguage` + language detection + extractors + shared tree-sitter helpers

Observed friction:

- Shared helpers exist in multiple places (ignore sets, rel-path normalization, discovery)
- The “deps” concept is split across `src/deps/*` and `src/index/deps.rs`
- `VaultIndex` currently mixes “vault facts” with “code import/workspace caches”
- Several signatures are drifting into “thread context through params” (clippy: too many arguments)

## Problems to solve

1) Ownership is unclear: which module is the canonical source of discovery/ignore/path semantics?
2) Duplicated utilities drift over time and produce inconsistent behavior across CLI/LSP/indexing.
3) Indexing is conceptually two indexes (markdown vault + code imports) but is represented as one type.
4) Wiring layers (CLI/LSP) own too much setup and repeat it.

## Proposal: shared components (conceptual)

### ProjectContext

A single struct responsible for:

- canonical root
- loaded config-derived ignore patterns
- compiled ignore matchers
- project-wide discovery helpers

Used by:

- CLI command implementations
- LSP backend

### Canonical shared utility modules

- `paths`: `normalize_rel_path`, slash-path conversions, safe rel-path extraction
- `ignore`: building ignore globsets + a single source of “always ignored” directories
- `discover`: walking the workspace once (or consistently) for both markdown/code and tree output
- `text`: line-start/offset helpers used by LSP + resolvers

### ProjectIndex split

Represent indexing as two explicit components:

- `VaultIndex`: markdown files, headings, links, inbound maps, vault checks
- `CodeIndex`: per-language import resolution results, workspace packages, workspace caches

CLI/LSP can hold a `ProjectIndex { vault, code }` (or a unified wrapper) rather than overloading `VaultIndex`.

### WorkspaceCaches

One struct holding the per-language caches (Go/Python/Rust/TSJS), instead of threading four separate cache params. This keeps signatures small and keeps caching policy in one place.
u
## Proposal: ideal `src/` layout (physical)

```text
src/
  main.rs
  lib.rs

  project/
    mod.rs            # ProjectContext
    root.rs           # root marker discovery (was src/root.rs)
    config.rs         # config loading (was src/config.rs)
    ignore.rs         # ignore sets + always-ignored dirs
    paths.rs          # normalize_rel_path + rel path helpers
    discover.rs       # single discovery/walk API
    text.rs           # line starts / offset helpers shared by LSP + resolve

  lang.rs             # CodeLanguage + language_for_path (currently in symbols)

  index/
    mod.rs            # ProjectIndex { vault, code }
    vault/
      mod.rs          # VaultIndex + check/link graph
      markdown.rs     # parse_markdown + slug/section bounds
      refs.rs         # inbound refs queries
      deps.rs         # markdown outbound deps queries
    code/
      mod.rs          # CodeIndex + ImportKind + ResolvedImport
      workspace.rs    # package discovery (pnpm/package.json)
      go.rs
      python.rs
      rust.rs
      tsjs.rs

  symbols/
    mod.rs            # extraction dispatch
    extract/
      go.rs
      python.rs
      rust.rs
      typescript.rs
    query.rs
    render.rs

  fmt/
    mod.rs
    preamble.rs

  tree/
    mod.rs            # TreeBuilder/TreeOptions + rendering

  cli/
    mod.rs            # command wiring
    context.rs        # CmdContext (thin wrapper over ProjectContext)

  lsp/
    mod.rs
    backend.rs
```

Notes:

- The proposed structure is a target. It can be achieved incrementally by introducing new modules, then moving code over with compatibility re-exports.
- The goal is to make “shared” code boring and obvious, and keep language-specific logic isolated.

## Non-goals

- No behavior changes required to accept this proposal.
- No one-shot big-bang move/rename of the entire tree.
- No public API break is required; re-exports can keep external paths stable during transition.

## Execution plan (incremental)

1) Introduce `src/lang.rs` and move `CodeLanguage` + `language_for_path` (aligns with iss-0039).
2) Introduce `src/project/{paths,ignore,discover}.rs` and migrate all call sites to a single canonical implementation (iss-0047).
3) Introduce `CmdContext` wrapping `ProjectContext` to eliminate repeated CLI setup (iss-0040).
4) Introduce `WorkspaceCaches` to fix “too many arguments” on resolver dispatch (iss-0043).
5) Move TS/JS workspace logic out of `resolve/mod.rs` (iss-0044).
6) Split `VaultIndex` into `ProjectIndex { vault, code }` (iss-0048).
7) Update `CODEMAP.md` once the structure is stable (iss-0016).

## Related issues

- iss-0039: Lift `CodeLanguage` to a neutral module
- iss-0040: Extract `CmdContext` in `cmd.rs`
- iss-0043: Consolidate resolve workspace caches
- iss-0044: Move TS/JS workspace logic out of `resolve/mod.rs`
- iss-0047: Introduce `project/` shared modules + `ProjectContext`
- iss-0048: Split vault indexing from code indexing (`ProjectIndex`)
- iss-0016: Codemap


## Done when

- The team agrees this is the target structure and boundaries.
- Sub-issues are updated/created to implement this incrementally.
- `CODEMAP.md` matches the adopted module layout once changes land.
