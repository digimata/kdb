# kdb

(knowledge database)

A language server and CLI for treating a markdown knowledge base as a compiled codebase.

## Core Analogy

| Code Concept         | Knowledge Equivalent                        |
|----------------------|---------------------------------------------|
| Module               | Markdown file                               |
| Exported symbol      | Heading (`## Definition`)                   |
| Import / reference   | Link (`[text](file.md#heading)`)            |
| Compile error        | Broken link (target file/heading missing)   |
| Dead code            | Orphan file (nothing links to it)           |
| Public API           | Outline (heading tree of a file)            |
| Type signature       | Frontmatter schema (structured metadata)    |
| Interface            | Template (expected structure for a category)|
| Dependency graph     | Link graph across files                     |

## Primitives

The system operates on a small set of primitives that map directly to markdown:

- **Files** — the unit of knowledge. One concept, one file (encouraged, not enforced).
- **Headings** — addressable anchors within a file. These are your "definitions."
- **Links** — references between files or to specific headings. These are your "imports."
- **Frontmatter** — structured metadata (YAML). Think of it as type annotations on the module.
- **Tags** — lightweight categorization. Like marker interfaces.

## Features

### 1. Compile / Check

Run a check across the entire knowledge base. Reports:

- **Broken links** — references to files or headings that don't exist.
- **Orphan files** — files with no inbound links (unreachable knowledge).
- **Empty stubs** — files that are referenced but have no content.
- **Frontmatter violations** — files that claim a `type:` but don't match the expected template.
- **Duplicate headings** — ambiguous jump-to-definition targets within a file.

Exit code 0 = clean knowledge base. Non-zero = issues found. CI-friendly.

```
$ kdb check
src/react.md:14 — broken link to "hooks.md#useReducer" (heading not found)
src/orphan.md — orphan file (0 inbound links)
src/distributed-systems.md — empty stub (referenced 3 times, no content)

3 errors, 0 warnings
```

### 2. Language Server (LSP)

Real-time feedback in any editor that supports LSP:

- **Diagnostics** — red squiggles on broken links, warnings on orphans.
- **Go to definition** — click a link, jump to the target file + heading.
- **Hover** — preview the first paragraph of the linked section.
- **Autocomplete** — start typing `[link](` and get completions for files and headings.
- **Rename symbol** — rename a heading, all links across the knowledge base update.
- **Find all references** — "who links to this heading?"
- **Outline view** — heading tree for the current file (standard LSP document symbols).
- **Code actions** — "create file" when linking to something that doesn't exist yet.

### 3. CLI

```
kdb check              # compile — report all errors/warnings
kdb outline <file>     # print heading tree
kdb refs <file>#<head> # find all references to a heading
kdb orphans            # list orphan files
kdb stubs              # list empty stubs
kdb graph              # output dependency graph (dot format)
kdb graph --cluster    # detect clusters of related knowledge
kdb init               # initialize a kdb project (creates .kdb/config.toml)
kdb fmt                # normalize link formats, fix slugs
```

### 4. Config (`.kdb/config.toml`)

```toml
[project]
name = "my-brain"
root = "."
ignore = ["archive/**", "drafts/**"]

[links]
style = "relative"         # "relative" | "wikilink"
require_heading_anchor = false

[templates.concept]
required_sections = ["## Definition", "## See Also"]
required_frontmatter = ["tags"]

[templates.project]
required_sections = ["## Status", "## Goals", "## Log"]
required_frontmatter = ["status", "start_date"]

[warnings]
orphans = true
stubs = true
max_file_length = 500      # lines — just a warning, not an error
```

## Link Syntax

Support both standard markdown links and wikilinks:

```markdown
# Standard
[React Hooks](react/hooks.md#useEffect)

# Wikilink (Obsidian-compatible)
[[react/hooks#useEffect]]
```

The LSP normalizes these internally. `kdb fmt` can convert between styles.

## Templates as Interfaces

If a file declares `type: concept` in frontmatter, the system checks it against the `[templates.concept]` config:

```markdown
---
type: concept
tags: [programming, react]
---

# React Hooks

## Definition
...

## See Also
- [[react/state]]
```

Missing a required section? That's a compile error — like failing to implement an interface.

## Architecture

```
kdb-cli (Rust)
  ├── parser       — markdown parsing, heading extraction, link extraction
  ├── resolver     — resolve links to targets, detect broken refs
  ├── checker      — orchestrate all checks, produce diagnostics
  ├── graph        — build + query the link graph
  ├── formatter    — normalize links, slugify headings
  └── lsp          — language server protocol implementation
```

Rust makes sense here: fast enough for real-time LSP on large vaults, single binary distribution, good markdown parsing ecosystem (pulldown-cmark).

## Non-Goals (for now)

- Not a note-taking app — this is tooling, not UI.
- Not a static site generator — but the graph/check data could feed one.
- Not Obsidian-specific — works on any directory of markdown files.
- No cloud sync — it's just files. Use git.

## Open Questions

- Should the graph be persisted (sqlite? flat file?) or rebuilt on each run?
- How to handle aliases (a file known by multiple names)?
- Transclusion support (`![[embed]]`)? Or out of scope?
- Should `kdb check` have severity levels (error vs warning) that are configurable?
- Worth supporting non-markdown files (e.g., linking to code files)?
