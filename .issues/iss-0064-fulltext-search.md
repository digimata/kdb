---
id: 64
title: "full-text search — kdb search <query> over md/text/code"
status: in_progress
priority: medium
labels:
  - feat
---

# iss-0064 :: Full-Text Search — `kdb search <query>`

## Problem

kdb indexes structure (headings, code symbols, refs, deps) and relational state (tasks/cycles), but there is no way to search the *content* of the corpus. To find "where did we write about the read-only ceiling" you fall back to `rg`, which gives unranked substring hits with no stemming and no integration with kdb's addressing model (`file#heading`, `file::symbol`) or its relational data.

## Proposal

Add a content full-text index and a `kdb search <query>` command. The enabling fact: kdb already links `rusqlite` with the `bundled` feature, whose SQLite ships **FTS5** (BM25 ranking, porter stemming, phrase/boolean/prefix queries, `snippet()`/`highlight()`) — so the search engine is already compiled in. **Zero new dependencies.** File discovery (`src/index/scanner.rs` + `workspace/ignore`), markdown parsing (tree-sitter-md), and code-symbol extraction already exist and are reused.

### MVP (this spike)

- Migration `0005_search.sql`: `CREATE VIRTUAL TABLE search_fts USING fts5(path, title, body, tokenize='porter unicode61')`.
- `src/search.rs`: `reindex()` walks md/text/code files via the existing scanner and upserts rows; `query()` runs `… MATCH ?1 ORDER BY rank`.
- `Search` subcommand in `main.rs` → `cmd::search` printing `path` + highlighted snippet.
- Freshness: full reindex on each search (simple, correct; the corpus is small).
- Granularity: file-level.

### Collections (scoping primitive)

Named, registered directories so search can be constrained:

```
kdb collection add ~/notes --name notes
kdb collection add ~/Documents/meetings --name meetings
kdb search "read-only ceiling" --collection notes
```

A `collections` table (`name`, `path`); `collection add|list|rm`; search takes `--collection <name>` (or `-C`) and filters indexed rows by that path prefix. Indexing can also be scoped to registered collections rather than the whole workspace. Included in the spike at minimal fidelity (table + add/list + `--collection` filter).

### Good version (follow-up)

- Section/symbol-level chunks (hits return `file.md#heading` / `file.rs::symbol`) using the parsers already present — this is what makes it better than `rg` and fits kdb's addressing.
- Per-file mtime/hash → incremental reindex instead of full rebuild.
- `--limit`, `--type md|code`, `--project`, JSON output.
- Optionally join against the relational layer (search a note → show linked tasks).

## Open decisions

1. **Freshness model:** explicit `kdb index --reindex` vs. lazy reindex-changed-on-search vs. piggyback on materialize. MVP uses full-rebuild-on-search; recommend lazy-incremental for the good version.
2. **Granularity:** file-level (MVP) vs. section/symbol-level (good version, recommended).
3. **Scope of indexed files:** which extensions count as "text" (md/txt/rs/py/js/ts/go + ?).

## Out of scope

Semantic/vector search (embeddings + `sqlite-vec`) is a separate, larger effort. FTS5 is lexical-only; this issue is keyword search, ranked.

## Effort

MVP ≈ half a day. Good version ≈ 1–2 days. Low risk: greenfield (no existing FTS), additive migration, command mirrors `refs`/`outline`.

## Status — spike landed

Shipped (`migrations 0005`+`0006`, `src/search.rs`, `kdb search`/`index`/`collection`):

- FTS5 index, BM25 ranking, porter stemming, highlighted snippets.
- **Incremental sync** keyed on `(mtime, size)` — unchanged files cost one `stat`; deleted files pruned. Steady-state ≈ 0.07 s for ~7.1k files; cold `--rebuild` ≈ 29 s over the whole monorepo.
- Restricted corpus: `json`/`yaml`/`yml` excluded (was 85% of bytes / pure data); hard 256 KB per-file cap.
- `--ftype docs|code|all`, **default `docs`** (md/markdown/txt); code opt-in, `[code]`-tagged.
- Named collections (`kdb collection add <dir> --name <n>`, `kdb search … -C <n>`).
- Input sanitized into safe FTS5 tokens (raw `read-only` no longer crashes).
- Friendly output: numbered, `rel <score>` relevance, header with ftype/collection.
- 8 unit tests; clippy-clean; uncommitted in `projects/kdb`.

Deferred (good version): section/symbol-level chunks (`file.md#heading`); project-scoped default indexing (cold rebuild still walks whole monorepo); per-collection extension overrides; `kdb search --json`; external-root collections (`~/notes`).
