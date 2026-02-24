---
id: 40
title: Replace Markdown Engine with Tree-Sitter
status: done
priority: high
labels:
  - feat
  - infra
  - markdown
---

# ISS-0040 :: Replace Markdown Engine with Tree-Sitter

## Context

- Markdown parsing is currently `pulldown-cmark` + regex logic in `src/index/mod.rs`, while the rest of kdb uses tree-sitter.
- Markdown behavior is fragmented across `src/index/mod.rs`, `src/lsp/hover.rs`, and command-specific logic, which creates drift and duplicated section-boundary rules.
- We want a clean foundation for future markdown features (structural transforms, precise ranges, shared section extraction, richer editor actions).
- Decision: remove the legacy markdown parser path and make tree-sitter markdown the only syntax engine.
- This is a prerequisite for `iss-0039`.

## Changes

1. Replace the markdown parser stack end-to-end
   - What file: `Cargo.toml`, `Cargo.lock`, `src/index/mod.rs`, `src/index/markdown.rs` (new)
   - What it does:
     - Add markdown tree-sitter grammar dependency.
     - Delete the `pulldown-cmark` event-walk parsing path from `src/index/mod.rs`.
     - Move markdown parsing into `src/index/markdown.rs` as the single syntax backend.
   - Key decisions or trade-offs:
     - Big refactor, but removes split architecture and parser mismatch permanently.

2. Build a first-class markdown semantic model on top of tree-sitter
   - What file: `src/index/markdown.rs`
   - What it does:
     - Emit headings with title, level, anchor, and precise line/byte ranges.
     - Emit link records with source ranges and normalized targets.
     - Keep code-span awareness in the parser pipeline so downstream features can trust extracted ranges.
   - Key decisions or trade-offs:
     - Higher upfront complexity for stronger long-term feature velocity.

3. Make wikilinks a first-class parse path
   - What file: `src/index/markdown.rs`
   - What it does:
     - Implement `[[...]]` extraction as part of the markdown parsing pipeline, not as a legacy regex fallback.
     - Ensure wikilinks in code blocks/inline code are excluded by parser-driven ranges.
   - Key decisions or trade-offs:
     - Requires custom handling beyond stock markdown grammar, but avoids fragile ad-hoc extraction.

4. Introduce canonical section/heading APIs
   - What file: `src/index/markdown.rs`, `src/index/mod.rs`
   - What it does:
     - Add shared helpers for slug matching and section bounds.
     - Define one boundary rule for all consumers: heading start to next heading of equal or higher level (or EOF).
   - Key decisions or trade-offs:
     - One source of truth eliminates hover/symbols divergence.

5. Migrate all markdown consumers to the new API
   - What file: `src/index/mod.rs`, `src/index/refs.rs`, `src/lsp/hover.rs`, `src/lsp/diagnostics.rs`, `src/lsp/completion.rs`, `src/lsp/definition.rs`, `src/symbols/query.rs`
   - What it does:
     - Move each consumer to the new markdown parser + helper APIs.
     - Remove local parsing/section logic from consumer modules.
   - Key decisions or trade-offs:
     - Larger blast radius now, cleaner architecture forever.

6. Delete legacy markdown code and dead helpers
   - What file: `src/index/mod.rs` (and any now-unused markdown helpers)
   - What it does:
     - Remove old parse pipeline, duplicate range helpers, and parser-specific workaround code that no longer applies.
     - Keep only the new tree-sitter-backed surface.
   - Key decisions or trade-offs:
     - No dual-path fallback; this intentionally forces a clean cutover.

7. Re-baseline tests on the new foundation
   - What file: `tests/index.rs`, `tests/lsp.rs`, `tests/cli.rs`
   - What it does:
     - Update/add tests for headings, links, wikilinks, anchor matching, and section bounds.
     - Add regression tests for parser edge cases that matter to kdb semantics.
   - Key decisions or trade-offs:
     - Re-baselining is expected; old parser-specific assumptions are not preserved just for compatibility.

## Files touched

```
┌────────────────────────┬───────────────────────────────────────────────────────────┐
│          File          │                          Action                           │
├────────────────────────┼───────────────────────────────────────────────────────────┤
│ Cargo.toml             │ Edit(add markdown tree-sitter, remove pulldown-cmark)    │
│ Cargo.lock             │ Edit(lockfile update)                                     │
│ src/index/markdown.rs  │ Create(tree-sitter markdown parser + semantic helpers)    │
│ src/index/mod.rs       │ Edit(remove legacy parser path, wire new module)          │
│ src/index/refs.rs      │ Edit(use shared markdown semantics helpers)               │
│ src/lsp/hover.rs       │ Edit(use shared section extraction APIs)                  │
│ src/lsp/diagnostics.rs │ Edit(use shared markdown parse outputs)                   │
│ src/lsp/completion.rs  │ Edit(use shared heading/anchor helpers)                   │
│ src/lsp/definition.rs  │ Edit(use shared target parsing APIs)                      │
│ src/symbols/query.rs   │ Edit(use shared markdown section helper path)             │
│ tests/index.rs         │ Edit(new parser/semantic contract coverage)               │
│ tests/lsp.rs           │ Edit(hover + link behavior on new parser)                 │
│ tests/cli.rs           │ Edit(markdown symbols/selector behavior on new parser)    │
└────────────────────────┴───────────────────────────────────────────────────────────┘
```

## Verification

- `cargo test --test index`
- `cargo test --test cli symbols_`
- `cargo test --test lsp`
- `cargo test`
- Confirm legacy parser removal:
  - no `pulldown-cmark` dependency in `Cargo.toml`
  - no old markdown event-walk parser in `src/index/mod.rs`
- Manual checks:
  - `kdb check` on a markdown-heavy fixture with wikilinks and nested headings
  - `kdb refs` for file + heading targets
  - LSP hover/definition/completion on markdown links and anchors
