---
id: 58
title: "root-anchored links and fmt navigation headers"
status: in_progress
priority: high
labels:
  - enhancement
  - fmt
path: qmd/.issues/iss-0058-root-links-and-fmt-nav.md
outline: |
  • iss-0058 :: Root-Anchored Links & Fmt Navigation Headers      L20
    ◦ Problem                                                     L22
    ◦ Solution                                                    L26
      ▪ 1. kdb:// root-anchored link scheme                       L30
      ▪ 2. kdb fmt navigation headers for markdown                L38
      ▪ Properties                                                L57
    ◦ Open Questions                                              L63
---

# iss-0058 :: Root-Anchored Links & Fmt Navigation Headers

## Problem

Relative links (`../../../README.md`) are fragile — they break on file moves, are hard to read, and require mental path arithmetic. Markdown files also lack the navigational context that `kdb fmt` already provides for code files (symbol index headers).

## Solution

Two pieces:

### 1. `kdb://` root-anchored link scheme

A project-root-relative link format: `kdb://hermaeus/research/competitors/exa/overview.md`

- Resolves from project root, never breaks on file moves within the tree
- Compact enough to use inline
- Reference: Tobi Ludke's qmd (`~/Documents/repos/qmd`) already implements `qmd://` — study his approach

### 2. `kdb fmt` navigation headers for markdown

Extend `kdb fmt` to generate two auto-maintained blocks for markdown files, mirroring what it already does for code files:

**Breadcrumb** — parent chain as a blockquote:

```md
> [hermaeus](kdb://hermaeus) · [research](kdb://hermaeus/research) · [competitors](kdb://hermaeus/research/competitors)
```

**Outline** — heading index with line numbers, same format as code symbol index:

```md
```

Both in blockquotes so Zed dims them visually. Links are cmd+clickable for jump-to-def navigation.

**For index files**, optionally auto-generate a children table (scan directory, read each file's `# title`).

### Properties

- Idempotent — running `kdb fmt` twice produces the same output
- Deterministic — breadcrumb + outline are purely a function of file position and headings
- Same pattern for code and markdown — one format to learn

## Open Questions

- How does Tobi's qmd resolve `qmd://` links? Custom URI handler? LSP? Editor extension?
- Should `kdb fmt` preserve hand-written `> See also:` blocks or only manage its own sections?
- Delimiter format — use the same `// ---` style as code, or the blockquote `> ---` variant?
