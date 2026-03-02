---
id: 36
title: Language plugin interface
status: proposed
priority: medium
labels:
  - refactor
  - lang
  - architecture
path: qmd/.issues/iss-0036-language-plugin-interface.md
outline: |
  • ISS-0036 :: Language plugin interface                                           L30
    ◦ Intent                                                                        L32
    ◦ Problem                                                                       L43
    ◦ Goals                                                                         L56
    ◦ Non-goals                                                                     L64
    ◦ Proposal                                                                      L69
      ▪ 1) Central registry                                                         L73
      ▪ 2) Capability-based plugin interface                                        L86
      ▪ 3) Replace WorkspaceCaches with a plugin-owned cache map                   L140
      ▪ 4) Route subsystems through the registry                                   L160
    ◦ Packaging options                                                            L172
      ▪ Option A (near-term): single crate, per-language modules                   L174
      ▪ Option B (medium-term): workspace with per-language crates + features      L182
    ◦ Migration plan                                                               L193
    ◦ Validation                                                                   L204
    ◦ Open questions                                                               L209
---

# ISS-0036 :: Language plugin interface

## Intent

Make code-language support modular and extensible by introducing a single per-language plugin/interface and a central registry.

Adding a new language should mean:

- implement one language module (or crate)
- register it in one place

...not editing `match CodeLanguage { ... }` across `symbols/`, `fmt/`, `resolve/`, `index/`, and `lsp/`.

## Problem

Language-specific behavior is currently scattered, which makes extension and correctness harder:

- Path detection lives in `src/lang.rs` (`CodeLanguage::from_path`), but TS/JS support is broader in other modules (e.g. `.mjs`, `.cjs`, `.mts`, `.cts`).
- Tree-sitter grammar selection is in `src/symbols/tree.rs`.
- Symbol extraction dispatch is in `src/symbols/mod.rs`.
- Formatting comment style + preamble detection is in `src/fmt/preamble.rs`.
- Import resolution dispatch is in `src/resolve/mod.rs` (plus per-language caches in `WorkspaceCaches`).
- Identifier-usage scanning rules are in `src/index/code_refs.rs`.

The net effect: “add a language” touches too many call sites, and subtle mismatches can occur when one module supports a file type that another module fails to discover.

## Goals

- One source of truth for language detection + extension sets.
- One source of truth for “language hooks” (symbols/fmt/imports/refs).
- Minimal surface area for adding a language.
- Keep static, compile-time composition (no runtime-loaded plugins).
- Enable future extraction of languages into optional crates/features.

## Non-goals

- Runtime-loaded dylib plugins.
- Full typechecking / semantic resolution (this is syntactic + workspace-aware resolution only).

## Proposal

Introduce a language plugin API and a registry that all subsystems go through.

### 1) Central registry

Add a module that owns discovery and dispatch, e.g. `src/languages/mod.rs`:

```rust
// Pseudocode
pub fn plugin_for_path(path: &Path) -> Option<&'static dyn LanguagePlugin>;
pub fn is_supported_code_path(path: &Path) -> bool;
pub fn supported_code_globs() -> &'static [&'static str];
```

All file discovery walkers (indexing and fmt) should use `is_supported_code_path()` rather than re-implementing extension checks.

### 2) Capability-based plugin interface

Avoid a single “god trait”. Model per-language behavior as a small set of capabilities.

Minimum set based on current code:

- `symbols` capability: tree-sitter grammar selection + symbol extraction
- `fmt` capability: comment prefix + preamble end detection
- `imports` capability: workspace cache build + import resolution
- `refs` capability: identifier usage classification rules

Sketch:

```rust
pub trait LanguagePlugin: Sync + Send {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    // discovery
    fn matches_path(&self, path: &Path) -> bool;

    // symbols
    fn tree_sitter_language(&self, path: &Path) -> tree_sitter::Language;
    fn extract_symbols(&self, path: &Path, source: &str) -> anyhow::Result<Vec<Symbol>>;

    // fmt
    fn comment_prefix(&self, path: &Path) -> &'static str;
    fn preamble_end_index(&self, path: &Path, lines: &[String]) -> usize;

    // imports
    fn build_workspace_cache(
        &self,
        root: &Path,
        ignore_patterns: &[String],
    ) -> anyhow::Result<Box<dyn std::any::Any + Send + Sync>>;

    fn resolve_imports(
        &self,
        root: &Path,
        source_file: &Path,
        source: &str,
        workspace_cache: &dyn std::any::Any,
    ) -> Vec<ResolvedImport>;

    // refs
    fn is_usage_identifier_kind(&self, node_kind: &str) -> bool;
}
```

Notes:

- `Any` is used only to avoid a central “workspace cache enum” that must be edited for every new language. The registry owns cache creation and downcasting.
- `path` is passed into hooks so a single plugin can support multiple file kinds (e.g. TS/TSX/JS/JSX) while choosing the correct grammar.

### 3) Replace `WorkspaceCaches` with a plugin-owned cache map

Today `resolve::WorkspaceCaches` grows fields as languages are added.

Replace with:

```rust
pub struct WorkspaceCaches {
    // Language id -> cache (type erased)
    caches: std::collections::HashMap<&'static str, Box<dyn Any + Send + Sync>>,
}
```

`build_workspace_import_index()` becomes:

- discover code files via registry
- compute the set of plugins present
- build each plugin cache once
- parse files in parallel, resolving imports by calling plugin hooks

### 4) Route subsystems through the registry

Refactor these to remove direct `match CodeLanguage` dispatch:

- `src/symbols/mod.rs` and `src/symbols/tree.rs`
- `src/fmt/preamble.rs` and `src/fmt/mod.rs`
- `src/resolve/mod.rs`
- `src/index/code_refs.rs` (usage identifier rules)
- `src/lsp/backend.rs` (supported-code gating)

The only place that should “know all languages” is the registry.

## Packaging options

### Option A (near-term): single crate, per-language modules

- Keep one crate (`kdb`).
- Add `src/languages/` with `rust.rs`, `python.rs`, `go.rs`, `tsjs.rs`.
- Registry is a simple static list.

Pros: fastest refactor; minimal Cargo complexity.

### Option B (medium-term): workspace with per-language crates + features

- `crates/kdb-core`: index/project infra + traits + shared types
- `crates/kdb-lang-rust`, `crates/kdb-lang-python`, ...: each language implementation + tree-sitter grammar deps
- `crates/kdb-cli`: binary + default language feature set

Pros: optional deps, faster builds, clearer boundaries.
Cons: more crates/feature wiring.

Recommendation: start with Option A to prove the interface, then split when the interface stabilizes.

## Migration plan

1. Add registry + plugin interfaces; implement plugins for existing languages without changing behavior.
2. Migrate one subsystem at a time:
   - `fmt` (lowest risk)
   - `symbols`
   - `resolve` (import indexing)
   - `index/code_refs` (usage rules)
   - LSP gating for code formatting
3. Delete/inline obsolete dispatch code paths (`CodeLanguage::from_path` becomes a thin wrapper over registry, or is removed).

## Validation

- `cargo test` should stay green throughout.
- Add focused tests for supported file detection (e.g. TS/JS: `*.mjs`, `*.cjs`, `*.mts`, `*.cts`, `*.d.ts`), ensuring discovery and formatting/indexing agree.

## Open questions

- Do we keep `CodeLanguage` as a public enum, or replace it with plugin ids (`&'static str`) and treat “language” as data-driven?
- How strict should `matches_path()` be for multi-suffix files like `*.d.ts`?
- Do we want separate plugins for JS/TS/TSX, or a single TS/JS plugin that internally selects grammar per path?
