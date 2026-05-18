-- Full-text search index (iss-0064).
--
-- FTS5 ships in the bundled SQLite linked via rusqlite's `bundled` feature,
-- so this adds keyword search (BM25 ranking, porter stemming, snippets)
-- with no new dependencies. The index is kept fresh incrementally: only
-- files whose (mtime, size) changed are re-read; deleted files are pruned.

CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
    path UNINDEXED,   -- workspace-relative path, queried with `=`/LIKE, not MATCH
    title,            -- file stem / first heading
    body,             -- full file text
    tokenize = 'porter unicode61'
);

-- Per-file index state for incremental sync: skip files whose mtime+size
-- are unchanged since the last sync, prune rows for files now gone.
CREATE TABLE IF NOT EXISTS search_meta (
    path  TEXT PRIMARY KEY,
    mtime INTEGER NOT NULL,   -- unix mtime (seconds)
    size  INTEGER NOT NULL    -- bytes
);

-- Named directories used to constrain a search (`--collection <name>`).
-- In the MVP, `path` is a workspace-relative prefix; rows are filtered by it
-- at query time rather than tagged at index time.
CREATE TABLE IF NOT EXISTS collections (
    name TEXT PRIMARY KEY,
    path TEXT NOT NULL
);
