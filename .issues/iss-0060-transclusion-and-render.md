---
id: 60
title: "transclusion includes and kdb render"
status: done
priority: high
labels:
  - enhancement
---

# iss-0060 :: Transclusion Includes & `kdb render`

## Problem

Procedures, definitions, and content blocks get duplicated across files. SOPs define canonical procedures, but scheduled tasks and prompts copy them verbatim. When the SOP changes, the copies go stale.

## Solution

### 1. Include syntax

Obsidian-style embed syntax:

```md
![[SOP.md#daily-shutdown]]
```

- `![[file]]` — embed entire file
- `![[file#heading]]` — embed a specific section (heading + its children)
- `![[kdb://file#heading]]` — root-relative embed
- Auto-appends `.md` if no extension (wikilink convention)

### 2. `kdb render` command

```bash
kdb render <file>
```

Reads a markdown file, resolves all `![[]]` embeds recursively, outputs the fully resolved content to stdout. No file modification — pure render-time resolution.

Use case: shell scripts pipe rendered output to claude:

```bash
claude -p "$(kdb render .tasks/sched/tr-001-morning-brief.md)"
```

### 3. `kdb check` validation

`kdb check` validates embed targets exist (file + optional heading anchor) alongside existing link validation.

## Implementation

- `src/render/include.rs` — regex-based parsing of `![[target]]` embeds
- `src/render/resolve.rs` — recursive resolution with cycle detection (max depth 10)
- `src/render/mod.rs` — public API: `render_file`, `render_content`
- `src/cmd.rs` — `render()` command function
- `src/main.rs` — `Render` CLI variant
- `src/index/mod.rs` — `BrokenEmbed` type, embed validation in `check()`
- `tests/render.rs` — 15 integration tests

## Resolved questions

- Includes are recursive (file A embeds file B which embeds file C) — yes
- Max depth 10 to prevent runaway recursion, cycle detection via visited set
- `kdb check` validates embed targets — yes
- No `kdb fmt` integration — embeds are pure render-time, files stay clean
