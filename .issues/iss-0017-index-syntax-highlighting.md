---
id: 17
title: Syntax Highlighting for Code Index Headers
status: proposed
priority: medium
labels:
  - feat
  - lsp
---

# 0017 :: Syntax Highlighting for Code Index Headers

## Intent

Make the `## Index` comment blocks visually match real code by highlighting keywords, symbol names, and line numbers with the editor's theme colors — instead of rendering everything in flat comment gray.

## Approach

### Option A: LSP semantic tokens (preferred)

The kdb LSP already runs in the editor. Add a `textDocument/semanticTokens` handler that detects index block lines and emits token spans:

| Token | Semantic type | Example |
|---|---|---|
| `fn`, `struct`, `enum`, `trait`, `type`, `class`, `interface` | `keyword` | `fn` in `fn init()` |
| Symbol names | `function` or `type` (depending on kind) | `init()`, `Backend::new()`, `CodeLanguage` |
| Line numbers | `number` | `L28` |

The editor theme handles the rest — keywords get keyword color, functions get function color, etc. Works in any editor supporting LSP semantic tokens (Zed, VSCode, Neovim).

### Option B: Tree-sitter injection queries (Zed/Neovim only)

Ship custom tree-sitter queries in the kdb Zed extension that recognize `// ## Index` blocks and inject syntax highlighting for the structured content inside. Zero runtime cost but editor-specific and more fragile.

## Recommendation

Start with Option A. It's portable, we already own the LSP, and semantic tokens are well-supported. Option B could be added later as a fast-path optimization for Zed.

## Changes

| File | Change |
|---|---|
| `src/lsp/mod.rs` | Register semantic tokens capability |
| `src/lsp/semantic_tokens.rs` (new) | Detect index blocks, emit token spans |
| `src/lsp/backend.rs` | Wire up `textDocument/semanticTokens/full` handler |

## Open questions

- Should we also highlight the `## Index` header line itself (e.g. as a heading/section)?
- Should the `//` comment prefix on each line retain comment coloring, with only the content portion highlighted?
