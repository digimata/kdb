---
id: 39
title: Lift CodeLanguage to src/lang.rs
status: done
priority: high
labels:
  - refactor
---

# ISS-0039 :: Lift CodeLanguage to src/lang.rs

## Intent

`CodeLanguage` is the foundational dispatch key for symbols, resolve, fmt, and the LSP — but it lives in `symbols/mod.rs` and everything else imports it from there. Give it its own top-level module so the dependency direction is clean.

## Scope

- Create `src/lang.rs` with `CodeLanguage` enum
- Move `language_for_path()` → `CodeLanguage::from_path()`
- Move `as_str()` (already an impl method)
- Keep tree-sitter mapping out of `src/lang.rs` (leave it in `symbols` as `tree_sitter_language(CodeLanguage)`)
- Update all imports across symbols, resolve, fmt, lsp, cmd

## Why

Aligns with iss-0046 by making `CodeLanguage` a shared crate-level type rather than a symbols-internal one, without introducing a `lang -> tree-sitter` dependency.

## Non-goals

- No logic changes. Pure code motion.
