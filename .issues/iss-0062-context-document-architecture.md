---
id: iss-0062
title: "Research Driver AI's context document architecture"
status: in_progress
priority: medium
labels: [research, architecture]
---

# iss-0062 — Research Driver AI's context document architecture

Driver AI (https://driver.ai, YC W24) builds "the context layer" for codebases — compiler-inspired architecture that produces structured context documents (architecture overviews, onboarding guides, changelogs, file-level symbol docs, navigable code maps).

## Research findings

Sources: driver.ai homepage, FAQs, blog index (blog posts are client-rendered, couldn't extract full text). No public repos or open-source components found. No docs site (docs.driver.ai returns DNS error).

### The "transpiler" — what it actually is

Driver calls their pipeline a **transpiler** — source code → structured context documents. The compiler analogy is deliberate and runs deep. The three pillars they advertise are:

1. **Transpiler** — the overall pipeline metaphor (code in, context docs out)
2. **Multi-pass refinement** — multiple passes over increasingly abstract representations
3. **Symbol-complete rigor** — exhaustive coverage of every symbol in the codebase

#### How it works (reconstructed from FAQs + landing page)

**Pass 0 — DAG construction.** The codebase is modeled as a directed acyclic graph (DAG). Nodes come from two sources:
- The **file tree** (files and folders)
- **Syntax trees and symbol tables** built via static analysis (for "specialized" languages: C/C++, Python, Java, Go, C#, JS, TS, Rust, Ruby, assembly, Verilog/SystemVerilog)
- For non-specialized languages, they still process but without static type info / rigorous symbol tables

**Pass 1 — Granular entity documentation.** Each node in the DAG gets documented individually. Because each unit is small, the LLM task is "significantly constrained" — they emphasize this shifts correctness burden from LLM inference to the decomposition structure. This is where they claim "content correct by construction."

**Pass N — Higher-level abstraction passes.** Subsequent passes build content at higher levels of abstraction on top of the foundational per-node docs. This is where architecture overviews, onboarding guides, etc. emerge.

**Multi-repo handling.** Multiple repositories are combined as disjoint graphs and processed together to build cross-repo content.

**Incremental updates.** SCM integration triggers recompilation on commits. They model their own operations "similar to git" for branch-level tracking.

Their key claim: decomposing the problem in **space** (granular units) and **time** (multiple passes) shifts correctness/exhaustiveness burden to the decomposition structure rather than LLM inference. This is contrasted with RAG, which they position as fundamentally inferior for this use case.

### Output taxonomy — the 6 document types

Exposed via MCP server with these tools:

| MCP tool | Document type | Description |
|---|---|---|
| `get_architecture_overview` | Architecture overview | Describes architecture of the whole codebase, "optimized to inform an LLM agent" |
| `get_llm_onboarding_guide` | Onboarding guide | Broad overview + navigation tips for quickly ramping an LLM agent |
| `get_changelog` | Changelog | Development history by year/month |
| `get_detailed_changelog` | Detailed changelog | Commit-log-derived detail for a specific month/year |
| `get_file_documentation` | File documentation | "Complete and exhaustive symbol-level documentation" for a given file path |
| `get_code_map` | Code map | "A navigable tree structure for the codebase returning terse descriptions and metadata for nodes" |

Plus `get_codebase_names` for discovery.

### Code maps — the most relevant concept for kdb

The code map is described as: **a navigable tree structure returning terse descriptions and metadata for nodes.** This is essentially `kdb tree` enhanced with per-node descriptions.

The key differences from what `kdb tree` does today:
- **Terse descriptions per node** — each file/directory in the tree has a generated one-liner describing what it does
- **Metadata per node** — likely includes symbol counts, language, last-modified, etc.
- **Navigable** — the tree is interactive/traversable (drill into subtrees)
- **Pre-computed** — descriptions are generated during compilation, not at query time

This maps directly to what a `kdb tree --describe` or `kdb map` feature could look like: the existing tree structure augmented with LLM-generated or statically-extracted per-node summaries.

## Ideas for kdb

### Code maps feature (informing kdb tree / new `kdb map`)

A code map for kdb could work as:
1. During `kdb fmt` or a new `kdb compile` pass, generate a one-line description for each file/directory
2. Store descriptions in `.kdb/` (e.g., `.kdb/map.json` or inline in index headers)
3. `kdb tree --describe` or `kdb map` renders the tree with descriptions
4. Incremental: only regenerate descriptions for changed files

This is feasible without LLMs for code files (use the existing symbol extraction to generate summaries like "defines struct Foo, impl Bar, 3 functions") and for markdown files (use the first heading or frontmatter title).

For LLM-enhanced descriptions, this could be an optional `kdb map --generate` mode.

### What we can skip

- **MCP server** — kdb already works as a CLI tool that agents call directly; MCP adds indirection without clear value for our use case
- **Multi-pass LLM refinement** — overkill for kdb's scope; our static analysis already covers symbol extraction
- **Deep context documents** (architecture overviews, onboarding guides) — these are higher-level outputs that depend on an LLM pipeline; not in kdb's lane

### What's genuinely interesting

- **The compilation metaphor itself** — you're right that this is basically compilation. Code → IR (DAG + symbol tables) → output artifacts (context docs). kdb already does the first two steps; the question is what output artifacts to add
- **Per-node descriptions in tree output** — high value, low complexity addition to `kdb tree`
- **Symbol-complete as a quality bar** — kdb already aims for this with `kdb symbols`; validates the approach

## Questions remaining

- What do their code map nodes actually look like? (can't tell without using the product)
- How do they handle description staleness / cache invalidation?
- Is the tree flat or hierarchically expandable via API?
