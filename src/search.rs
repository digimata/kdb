//! Full-text content search over the workspace (iss-0064).
//!
//! Uses SQLite FTS5 (shipped in the bundled SQLite via rusqlite's `bundled`
//! feature) for BM25-ranked, porter-stemmed keyword search with snippet
//! highlighting.
//!
//! The index is kept fresh **incrementally**: [`sync`] walks the workspace
//! (cheap — parallel, `.kdb/ignore`-aware), but only files whose `(mtime,
//! size)` changed since the last sync are re-read and re-tokenized; rows for
//! deleted files are pruned. Unchanged files cost a single `stat`, not a
//! read. A full [`rebuild`] is available via `kdb index --rebuild`.
//!
//! Each row is tagged with a [`FType`] class so search defaults to prose
//! (`docs`) and code is opt-in via `--ftype code|all`.

use anyhow::{Context, Result};
use globset::GlobSet;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::workspace::discover::discover_files;

// -----------------------------------------------
// projects/kdb/src/search.rs
//
// const DOC_EXTS                              L56
// const CODE_EXTS                             L63
// const MAX_FILE_BYTES                        L67
// fn classify()                               L70
// pub enum FType                              L83
//   fn kind_filter()                          L95
// pub struct Hit                             L106
// pub struct SyncStats                       L115
//   pub fn is_noop()                         L124
// pub fn rebuild()                           L130
// pub fn sync()                              L138
// pub fn resolve_collection()                L223
// pub fn terms()                             L239
// fn sanitize()                              L247
// pub fn query()                             L256
// pub fn collection_add()                    L295
// pub fn collection_list()                   L305
// mod tests                                  L314
// fn setup()                                 L321
// fn sync_is_incremental_and_prunes()        L329
// fn ftype_scopes_docs_vs_code()             L364
// fn collection_constrains_results()         L383
// fn sanitize_neutralizes_fts_operators()    L400
// fn raw_punctuation_query_is_safe()         L408
// fn json_excluded_and_oversize_skipped()    L420
// -----------------------------------------------

/// Prose / knowledge-base extensions — the default search scope.
const DOC_EXTS: &[&str] = &["md", "markdown", "txt"];

/// Code & config extensions — indexed, but opt-in at query time.
///
/// Deliberately excludes `json`/`yaml`/`yml`: in practice these are
/// transcript/benchmark/build data, ~85%+ of corpus bytes and pure
/// noise/cost for full-text search.
const CODE_EXTS: &[&str] = &["rs", "py", "js", "jsx", "ts", "tsx", "go", "toml", "sql", "sh"];

/// Hard per-file size cap. Hand-written docs/code are far below this; the
/// cap catches stray generated files regardless of extension.
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// Classify a path into its search class, or `None` if not indexable.
fn classify(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if DOC_EXTS.contains(&ext.as_str()) {
        Some("docs")
    } else if CODE_EXTS.contains(&ext.as_str()) {
        Some("code")
    } else {
        None
    }
}

/// Which file classes a search covers. Defaults to [`FType::Docs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum FType {
    /// Prose only: md / markdown / txt (default).
    #[default]
    Docs,
    /// Code & config only.
    Code,
    /// Everything indexed.
    All,
}

impl FType {
    /// The `kind` value to filter on, or `None` for no filter (`All`).
    fn kind_filter(self) -> Option<&'static str> {
        match self {
            FType::Docs => Some("docs"),
            FType::Code => Some("code"),
            FType::All => None,
        }
    }
}

/// A single search hit: path, class, highlighted excerpt, relevance score
/// (higher is a better match).
pub struct Hit {
    pub path: String,
    pub kind: String,
    pub snippet: String,
    pub score: f64,
}

/// Outcome of an incremental [`sync`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
}

impl SyncStats {
    /// True when the index already matched the workspace (no work done).
    pub fn is_noop(&self) -> bool {
        self.added == 0 && self.updated == 0 && self.removed == 0
    }
}

/// Drop the entire index so the next [`sync`] re-reads everything.
pub fn rebuild(conn: &Connection) -> Result<()> {
    conn.execute_batch("DELETE FROM search_fts; DELETE FROM search_meta;")?;
    Ok(())
}

/// Incrementally bring the FTS index in line with the workspace.
///
/// Only changed/new files are read; deleted files are pruned. Returns counts.
pub fn sync(conn: &Connection, root: &Path, ignore_set: &GlobSet) -> Result<SyncStats> {
    // Previous index state: path -> (mtime, size).
    let mut prior: HashMap<String, (i64, i64)> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT path, mtime, size FROM search_meta")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?;
        for row in rows {
            let (p, ms) = row?;
            prior.insert(p, ms);
        }
    }

    let files = discover_files(root, root, ignore_set)?;
    let tx = conn.unchecked_transaction()?;
    let mut stats = SyncStats::default();
    let mut seen: Vec<String> = Vec::new();

    {
        let mut del = tx.prepare("DELETE FROM search_fts WHERE path = ?1")?;
        let mut ins = tx
            .prepare("INSERT INTO search_fts (path, kind, title, body) VALUES (?1, ?2, ?3, ?4)")?;
        let mut meta = tx.prepare(
            "INSERT INTO search_meta (path, mtime, size) VALUES (?1, ?2, ?3) \
             ON CONFLICT(path) DO UPDATE SET mtime = excluded.mtime, size = excluded.size",
        )?;

        for rel in &files {
            let Some(kind) = classify(rel) else {
                continue;
            };
            let abs = root.join(rel);
            let Ok(md) = std::fs::metadata(&abs) else {
                continue;
            };
            // Over the cap: skip without adding to `seen`, so a file that
            // grew past the limit is evicted by the prune step below.
            if md.len() > MAX_FILE_BYTES {
                continue;
            }
            let mtime = md
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let size = md.len() as i64;
            let key = rel.to_string_lossy().to_string();
            seen.push(key.clone());

            if let Some(&(pm, ps)) = prior.get(&key) {
                if pm == mtime && ps == size {
                    stats.unchanged += 1;
                    continue; // single stat, no read — the whole point
                }
            }
            // New or changed: re-read and replace its rows.
            let Ok(body) = std::fs::read_to_string(&abs) else {
                continue; // binary / unreadable
            };
            let title = rel.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            del.execute([&key])?;
            ins.execute(rusqlite::params![key, kind, title, body])?;
            meta.execute(rusqlite::params![key, mtime, size])?;
            if prior.contains_key(&key) {
                stats.updated += 1;
            } else {
                stats.added += 1;
            }
        }

        // Prune files that disappeared since the last sync.
        let present: std::collections::HashSet<&String> = seen.iter().collect();
        let mut del_meta = tx.prepare("DELETE FROM search_meta WHERE path = ?1")?;
        for gone in prior.keys().filter(|p| !present.contains(p)) {
            del.execute([gone])?;
            del_meta.execute([gone])?;
            stats.removed += 1;
        }
    }

    tx.commit()?;
    Ok(stats)
}

/// Resolve a collection name to its stored workspace-relative path prefix.
pub fn resolve_collection(conn: &Connection, name: &str) -> Result<String> {
    conn.query_row("SELECT path FROM collections WHERE name = ?1", [name], |r| {
        r.get(0)
    })
    .with_context(|| format!("no collection named '{name}' (see `kdb collection list`)"))
}

/// Turn arbitrary user input into a safe FTS5 query.
///
/// Raw input is unsafe: FTS5 reads `-`, `:`, `*`, `^`, `NOT`/`AND`/`OR`,
/// and `(` as operators (e.g. `read-only` parses as `read NOT only`, and
/// `only:` as a column filter). We tokenize on non-alphanumerics and emit
/// each token as a quoted FTS5 string, AND-ed together — i.e. "documents
/// containing all these words", which is what a search box should do.
/// Lowercased alphanumeric tokens from arbitrary input. Shared by the FTS
/// sanitizer and the line-context matcher so they agree on "a term".
pub fn terms(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

fn sanitize(input: &str) -> String {
    terms(input)
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Run a full-text query, scoped by file class and optional path prefix.
pub fn query(
    conn: &Connection,
    needle: &str,
    ftype: FType,
    prefix: Option<&str>,
    limit: i64,
) -> Result<Vec<Hit>> {
    let fts = sanitize(needle);
    if fts.is_empty() {
        return Ok(Vec::new());
    }
    // `-bm25` so a larger score means a better match (friendlier to read).
    let sql = "SELECT path, kind, snippet(search_fts, 3, '«', '»', ' … ', 12), \
                      -bm25(search_fts) AS score \
               FROM search_fts \
               WHERE search_fts MATCH ?1 \
                 AND (?2 IS NULL OR kind = ?2) \
                 AND (?3 IS NULL OR path LIKE ?3 || '%') \
               ORDER BY rank \
               LIMIT ?4";
    let mut stmt = conn.prepare(sql)?;
    let hits = stmt
        .query_map(
            rusqlite::params![fts, ftype.kind_filter(), prefix, limit],
            |r| {
                Ok(Hit {
                    path: r.get(0)?,
                    kind: r.get(1)?,
                    snippet: r.get(2)?,
                    score: r.get(3)?,
                })
            },
        )
        .context("full-text query failed (check FTS5 query syntax)")?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(hits)
}

/// Register (or update) a named collection by workspace-relative prefix.
pub fn collection_add(conn: &Connection, name: &str, rel_path: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO collections (name, path) VALUES (?1, ?2) \
         ON CONFLICT(name) DO UPDATE SET path = excluded.path",
        rusqlite::params![name, rel_path],
    )?;
    Ok(())
}

/// List registered collections as `(name, path)` pairs, ordered by name.
pub fn collection_list(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT name, path FROM collections ORDER BY name")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::workspace::WorkspaceContext;
    use crate::workspace::root::ROOT_MARKER;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Connection) {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(ROOT_MARKER)).unwrap();
        let conn = db::open(tmp.path()).unwrap();
        (tmp, conn)
    }

    #[test]
    fn sync_is_incremental_and_prunes() {
        let (tmp, conn) = setup();
        std::fs::write(
            tmp.path().join("alpha.md"),
            "# Alpha\nThe read-only ceiling is the core observation.",
        )
        .unwrap();
        std::fs::write(tmp.path().join("beta.md"), "Unrelated content about goats.").unwrap();
        std::fs::write(tmp.path().join("skip.bin"), [0u8, 159, 146, 150]).unwrap();
        let ctx = WorkspaceContext::discover(tmp.path()).unwrap();

        let s1 = sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();
        assert_eq!(s1.added, 2);
        assert_eq!(s1.updated + s1.removed, 0);

        // No-op when nothing changed (the key property).
        let s2 = sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();
        assert!(s2.is_noop(), "unchanged workspace must do zero work: {s2:?}");
        assert_eq!(s2.unchanged, 2);

        let hits = query(&conn, "ceiling", FType::Docs, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "alpha.md");
        assert_eq!(hits[0].kind, "docs");
        assert!(hits[0].snippet.contains('«'));
        assert!(hits[0].score.is_finite());
        assert_eq!(query(&conn, "observ*", FType::Docs, None, 10).unwrap().len(), 1);

        std::fs::remove_file(tmp.path().join("beta.md")).unwrap();
        let s3 = sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();
        assert_eq!(s3.removed, 1);
        assert_eq!(query(&conn, "goats", FType::Docs, None, 10).unwrap().len(), 0);
    }

    #[test]
    fn ftype_scopes_docs_vs_code() {
        let (tmp, conn) = setup();
        std::fs::write(tmp.path().join("notes.md"), "the substrate thesis").unwrap();
        std::fs::write(tmp.path().join("lib.rs"), "// the substrate thesis\nfn x() {}").unwrap();
        let ctx = WorkspaceContext::discover(tmp.path()).unwrap();
        sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();

        let docs = query(&conn, "substrate", FType::Docs, None, 10).unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].path, "notes.md");

        let code = query(&conn, "substrate", FType::Code, None, 10).unwrap();
        assert_eq!(code.len(), 1);
        assert_eq!(code[0].path, "lib.rs");

        assert_eq!(query(&conn, "substrate", FType::All, None, 10).unwrap().len(), 2);
    }

    #[test]
    fn collection_constrains_results() {
        let (tmp, conn) = setup();
        std::fs::create_dir_all(tmp.path().join("research")).unwrap();
        std::fs::write(tmp.path().join("research/note.md"), "substrate thesis").unwrap();
        std::fs::write(tmp.path().join("root.md"), "substrate thesis").unwrap();
        let ctx = WorkspaceContext::discover(tmp.path()).unwrap();
        sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();

        collection_add(&conn, "research", "research").unwrap();
        let prefix = resolve_collection(&conn, "research").unwrap();
        let hits = query(&conn, "substrate", FType::Docs, Some(&prefix), 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "research/note.md");
        assert!(resolve_collection(&conn, "nope").is_err());
    }

    #[test]
    fn sanitize_neutralizes_fts_operators() {
        assert_eq!(sanitize("read-only ceiling"), "\"read\" \"only\" \"ceiling\"");
        assert_eq!(sanitize("  C++  "), "\"c\"");
        assert_eq!(sanitize("NOT (foo OR bar)"), "\"not\" \"foo\" \"or\" \"bar\"");
        assert!(sanitize("   ***   ").is_empty());
    }

    #[test]
    fn raw_punctuation_query_is_safe() {
        let (tmp, conn) = setup();
        std::fs::write(tmp.path().join("a.md"), "The read-only ceiling welded shut.").unwrap();
        let ctx = WorkspaceContext::discover(tmp.path()).unwrap();
        sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();
        // Previously errored: "no such column: only".
        let hits = query(&conn, "read-only ceiling", FType::Docs, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(query(&conn, "@#$%", FType::Docs, None, 10).unwrap().is_empty());
    }

    #[test]
    fn json_excluded_and_oversize_skipped() {
        let (tmp, conn) = setup();
        std::fs::write(tmp.path().join("data.json"), r#"{"substrate":"thesis"}"#).unwrap();
        std::fs::write(
            tmp.path().join("big.md"),
            format!("substrate {}", "x".repeat((MAX_FILE_BYTES + 1) as usize)),
        )
        .unwrap();
        std::fs::write(tmp.path().join("ok.md"), "substrate thesis").unwrap();
        let ctx = WorkspaceContext::discover(tmp.path()).unwrap();

        let s = sync(&conn, &ctx.root, &ctx.ignore_set).unwrap();
        assert_eq!(s.added, 1, "only ok.md indexed (json + oversize skipped)");
        let hits = query(&conn, "substrate", FType::All, None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "ok.md");
    }
}
