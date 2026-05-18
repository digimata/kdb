-- Add a file-class column to the search index so search defaults to prose
-- (`docs`) and code is opt-in via `--ftype` (iss-0064).
--
-- FTS5 virtual-table columns cannot be added with ALTER, so the table is
-- recreated and search_meta is cleared to force a one-time full reindex on
-- the next `kdb index` / `kdb search`.

DROP TABLE IF EXISTS search_fts;

CREATE VIRTUAL TABLE search_fts USING fts5(
    path UNINDEXED,   -- workspace-relative path
    kind UNINDEXED,   -- 'docs' | 'code'
    title,
    body,
    tokenize = 'porter unicode61'
);

DELETE FROM search_meta;
