---
id: 42
title: Extract symbol Extractor context struct
status: proposed
priority: medium
labels:
  - refactor
---

# ISS-0042 :: Extract Symbol Extractor Context Struct

## Intent

Per-language symbol extractors thread `&mut Vec<Symbol>` and `&mut SeenSymbols` through every call. `SeenSymbols` is an opaque 8-element tuple type. The shared helpers in `symbols/mod.rs` (`push_symbol`, `name_from_field`, `normalized_node_text`) all take `source: &[u8]` as a parameter.

## Scope

- Create `Extractor<'src>` struct holding `source`, `symbols`, and `seen`
- Move `push_symbol` → `Extractor::push`
- Move `name_from_field` → `Extractor::name_from_field`
- Move `normalized_node_text` → `Extractor::node_text`
- Add `Extractor::finish(self) -> Vec<Symbol>` to consume and return
- Kill the `SeenSymbols` type alias — dedup logic lives inside `Extractor::push`
- Update all per-language extractors (including any future split into `symbols/extract/*` per iss-0046)

## Why

Reduces parameter threading, kills the fragile tuple type, and satisfies CC-3.3. Each language extractor becomes cleaner — `ext.push(...)` instead of `push_symbol(&mut symbols, &mut seen, ...)`.
