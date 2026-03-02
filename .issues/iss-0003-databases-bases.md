---
id: 3
title: Databases and Bases
status: proposed
priority: high
labels:
  - roadmap
  - spec
path: qmd/.issues/iss-0003-databases-bases.md
outline: |
  • ISS-0003 :: Databases and Bases       L24
    ◦ Product Shape                       L28
      ▪ Indexed Notes DB (Derived)        L30
      ▪ Base/View Definitions             L41
      ▪ Renderer Targets                  L54
    ◦ Link Behavior                       L60
    ◦ Initial Schema Sketch               L76
    ◦ Rollout                            L116
    ◦ Non-Goals (Initial)                L124
    ◦ Done When                          L130
    ◦ Open Questions                     L136
---

# ISS-0003 :: Databases and Bases

Use SQLite to power Bases-style views over markdown notes while keeping markdown as the source of truth.

## Product Shape

### Indexed Notes DB (Derived)

Build `.kdb/index.db` from vault content.

- `files` table: path, title, mtime, size.
- `headings` table: file, level, text, anchor, order.
- `links` table: source, destination file, destination anchor, status.
- `properties` table: frontmatter and inferred metadata.

The DB is regenerated or incrementally updated from markdown. It is not the canonical source.

### Base/View Definitions

Define views in `.base` files (or embedded blocks).

```yaml
source: "notes/**/*.md"
select: [file, title, status, owner, updated]
where: "status != 'done'"
sort: ["updated desc"]
view: table
limit: 200
```

### Renderer Targets

- CLI table/list/cards output.
- JSON output for agents.
- LSP hover/peek previews for links to DB/base resources.

## Link Behavior

Support `[[file.db]]` as a first-class UX:

- `[[file.db]]` resolves to a sqlite file.
- Hover previews top rows with truncation.
- Go-to-definition opens DB (or base file) location.

For richer embeds, support explicit block syntax:

```kdb
source: books.db
query: SELECT title, author, rating FROM books ORDER BY rating DESC LIMIT 20;
view: table
```

## Initial Schema Sketch

```sql
CREATE TABLE files (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  title TEXT,
  mtime_ms INTEGER NOT NULL
);

CREATE TABLE headings (
  id INTEGER PRIMARY KEY,
  file_id INTEGER NOT NULL,
  level INTEGER NOT NULL,
  text TEXT NOT NULL,
  anchor TEXT NOT NULL,
  ord INTEGER NOT NULL,
  FOREIGN KEY(file_id) REFERENCES files(id)
);

CREATE TABLE links (
  id INTEGER PRIMARY KEY,
  src_file_id INTEGER NOT NULL,
  src_line INTEGER NOT NULL,
  raw TEXT NOT NULL,
  dest_path TEXT,
  dest_anchor TEXT,
  status TEXT NOT NULL,
  FOREIGN KEY(src_file_id) REFERENCES files(id)
);

CREATE TABLE properties (
  file_id INTEGER NOT NULL,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  PRIMARY KEY(file_id, key),
  FOREIGN KEY(file_id) REFERENCES files(id)
);
```

## Rollout

1. Add SQLite index writer for current parser output (full rebuild first).
2. Add `kdb db query --sql "..." --json|table`.
3. Add `.base` parser + `kdb base run <view.base>`.
4. Add LSP hover preview for `[[*.db]]` and `[[*.base]]`.
5. Add incremental indexing + cache invalidation.

## Non-Goals (Initial)

- No remote DB.
- No write-back to markdown from DB edits in first pass.
- No full dashboard UI in first pass; start with CLI + hover previews.

## Done When

- A derived `.kdb/index.db` can be built from vault files.
- Query and view definitions can render table/list outputs.
- Links to DB/base resources work in CLI and editor previews.

## Open Questions

- Should `.base` be YAML, TOML, or SQL-first?
- Should formulas be SQL-only initially, or have an expression language?
- How should embedded views in markdown cache and refresh?
