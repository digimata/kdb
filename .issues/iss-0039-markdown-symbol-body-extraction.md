---
id: 39
title: Markdown Symbol Body Extraction
status: proposed
priority: high
labels:
  - feat
---

# ISS-0039 :: Markdown Symbol Body Extraction

## Context

- `iss-0040` establishes the new markdown foundation (tree-sitter parser + shared heading/section APIs) and lands first.
- This issue is now focused only on enabling `kdb symbols <file> -s <symbol>` for markdown using that new foundation.
- Current failure remains in `src/symbols/query.rs`: markdown targets still error with `symbol body extraction is only supported for code files`.
- Desired behavior: selector matches heading slug and returns that section body with canonical section boundaries.

## Changes

1. Enable markdown selector extraction in `symbols -s`
   - What file: `src/symbols/query.rs`
   - What it does:
     - Replace markdown rejection branch in `collect_body_rows()` with markdown section extraction using shared helpers from `iss-0040`.
     - Accept slug selectors case-insensitively, including optional leading `#`.
     - Match against heading anchor slug, not raw heading text.
   - Key decisions or trade-offs:
     - Selector semantics are explicit slug-first behavior (`getting-started`), not fuzzy title matching.

2. Return markdown rows through existing body output channel
   - What file: `src/symbols/query.rs`, `src/symbols/render.rs` (if helper needed)
   - What it does:
     - Emit `SymbolBodyRow` entries for markdown with correct `kind`, `line`, `end_line`, and `body`.
     - Keep text output behavior as raw extracted section content.
   - Key decisions or trade-offs:
     - Keep JSON shape stable for now; markdown-specific metadata can be a separate enhancement.

3. Add CLI coverage for markdown `-s`
   - What file: `tests/cli.rs`
   - What it does:
     - Replace markdown rejection test with success-path tests.
     - Verify slug selector matching, case-insensitive matching, and section boundary behavior (next equal/higher heading).
     - Verify JSON output and not-found error behavior.
   - Key decisions or trade-offs:
     - Integration tests are preferred because they validate command wiring + extraction + rendering end to end.

## Files touched

```
┌────────────────────────┬───────────────────────────────────────────────────────────┐
│          File          │                          Action                           │
├────────────────────────┼───────────────────────────────────────────────────────────┤
│ src/symbols/query.rs   │ Edit(enable markdown `symbols -s` extraction path)        │
│ src/symbols/render.rs  │ Edit(optional helper for markdown SymbolBodyRow shaping)  │
│ tests/cli.rs           │ Edit(add markdown selector/body extraction coverage)       │
└────────────────────────┴───────────────────────────────────────────────────────────┘
```

## Verification

- Markdown symbol body extraction:
  - `cargo test --test cli symbols_selector_`
  - `cargo test --test cli symbols_`
- Full regression pass:
  - `cargo test`
- Manual checks:
  - `kdb symbols ~/.claude/SOP.md -s "sop-3-refactor-cleanup"`
  - `kdb symbols ~/.claude/SOP.md -s "SOP-3-REFACTOR-CLEANUP"`
  - Confirm output includes nested headings under the target and stops at the next heading of equal/higher level.
