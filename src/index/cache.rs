//! Persistent disk-backed index cache.
//!
//! Serializes per-file parse results (headings, links, symbols, imports) to
//! `.kdb/index.bin` and incrementally rebuilds — only re-parsing files whose
//! mtime/size changed. Design follows ruff's caching pattern.

use anyhow::{Context, Result};
use bincode::{Decode, Encode};
use indicatif::ProgressBar;
use rayon::prelude::*;
use seahash::SeaHasher;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::index::{FileEntry, Heading, Link, LinkKind, LinkTarget};
use crate::lang::CodeLanguage;
use crate::project::discover::discover_files;
use crate::project::ignore::{ALWAYS_IGNORED_DIRS, build_ignore_globset};
use crate::resolve::{
    ImportKind, ImportNames, ResolvedImport, WorkspaceCaches, resolve_imports_for_language,
};
use crate::symbols::{Symbol, SymbolKind, extract_symbols};

// ------------------------------------------------
// src/index/cache.rs
//
// const CACHE_FILE                             L63
// const CACHE_VERSION                          L64
// const GC_MAX_AGE_MILLIS                      L65
// struct IndexCache                            L69
//   fn load()                                  L80
//   fn save()                                  L92
//   fn gc()                                   L107
// struct CachedFileFacts                      L119
// enum CachedFileKind                         L130
// struct CachedHeading                        L143
// struct CachedLink                           L153
// struct CachedSymbol                         L164
// struct CachedImport                         L178
// struct CachedImportNames                    L188
// fn file_key()                               L199
// fn manifest_key()                           L212
// fn now_millis()                             L231
// pub(crate) struct IncrementalBuildResult    L243
// pub(crate) fn incremental_build()           L258
// fn to_heading()                             L460
// fn to_link()                                L470
// fn to_symbol()                              L487
// fn to_import()                              L501
// fn from_heading()                           L519
// fn from_link()                              L529
// fn from_symbol()                            L543
// fn from_import()                            L557
// fn symbol_kind_to_u8()                      L575
// fn symbol_kind_from_u8()                    L597
// fn import_kind_to_u8()                      L620
// fn import_kind_from_u8()                    L629
// ------------------------------------------------

const CACHE_FILE: &str = "index.bin";
const CACHE_VERSION: &str = env!("CARGO_PKG_VERSION");
const GC_MAX_AGE_MILLIS: u64 = 30 * 24 * 60 * 60 * 1000; // 30 days

/// On-disk cache: project root + per-file facts.
#[derive(Encode, Decode)]
struct IndexCache {
    /// kdb version that wrote this cache.
    version: String,
    /// Hash of workspace manifest mtimes. Change → full rebuild.
    manifest_key: u64,
    /// Per-file cached parse results.
    files: HashMap<PathBuf, CachedFileFacts>,
}

impl IndexCache {
    /// Load cache from `.kdb/index.bin`. Returns `None` on missing/corrupt/version mismatch.
    fn load(root: &Path) -> Option<Self> {
        let path = root.join(".kdb").join(CACHE_FILE);
        let bytes = fs::read(&path).ok()?;
        let (cache, _): (Self, _) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).ok()?;
        if cache.version != CACHE_VERSION {
            return None;
        }
        Some(cache)
    }

    /// Atomic write to `.kdb/index.bin` via tempfile + persist.
    fn save(&self, root: &Path) -> Result<()> {
        let dir = root.join(".kdb");
        assert!(dir.is_dir(), ".kdb directory must exist");

        let bytes =
            bincode::encode_to_vec(self, bincode::config::standard()).context("bincode encode")?;

        let mut tmp = tempfile::NamedTempFile::new_in(&dir).context("create temp file")?;
        std::io::Write::write_all(&mut tmp, &bytes).context("write cache")?;
        tmp.persist(dir.join(CACHE_FILE))
            .context("persist cache file")?;
        Ok(())
    }

    /// Prune entries not seen in 30 days.
    fn gc(&mut self) {
        let cutoff = now_millis().saturating_sub(GC_MAX_AGE_MILLIS);
        self.files.retain(|_, facts| facts.last_seen >= cutoff);
    }
}

// ---------------------------------------------------------------------------
// Cached mirror types
// ---------------------------------------------------------------------------

/// Cached parse artifacts for a single source file.
#[derive(Encode, Decode)]
struct CachedFileFacts {
    /// Staleness key: hash of (mtime, size).
    key: u64,
    /// Millis since epoch when this entry was last used.
    last_seen: u64,
    /// What kind of file produced these facts.
    kind: CachedFileKind,
}

/// Discriminated union of cached file types.
#[derive(Encode, Decode)]
enum CachedFileKind {
    Markdown {
        headings: Vec<CachedHeading>,
        links: Vec<CachedLink>,
    },
    Code {
        symbols: Vec<CachedSymbol>,
        imports: Vec<CachedImport>,
    },
}

/// Cached heading — mirrors [`Heading`].
#[derive(Encode, Decode)]
struct CachedHeading {
    title: String,
    anchor: String,
    level: u8,
    line: u32,
    column: u32,
}

/// Cached link — mirrors [`Link`].
#[derive(Encode, Decode)]
struct CachedLink {
    kind: u8, // 0 = Markdown, 1 = Wikilink
    raw: String,
    target_file: Option<String>,
    target_anchor: Option<String>,
    line: u32,
    column: u32,
}

/// Cached symbol — mirrors [`Symbol`].
#[derive(Encode, Decode)]
struct CachedSymbol {
    name: String,
    parent: Option<String>,
    kind: u8,
    display_kind: String,
    line: u32,
    end_line: u32,
    start_byte: u32,
    end_byte: u32,
    is_public: bool,
}

/// Cached import — mirrors [`ResolvedImport`].
#[derive(Encode, Decode)]
struct CachedImport {
    raw: String,
    resolved_path: Option<PathBuf>,
    kind: u8, // 0=Relative, 1=Workspace, 2=TsconfigPath, 3=External
    names: CachedImportNames,
    line: u32,
}

/// Cached import names — mirrors [`ImportNames`].
#[derive(Encode, Decode)]
struct CachedImportNames {
    locals: Vec<String>,
    aliases: Vec<(String, String)>,
    is_namespace: bool,
}

// ---------------------------------------------------------------------------
// Staleness key computation
// ---------------------------------------------------------------------------

/// Compute a staleness key from file metadata: hash of (mtime_secs, mtime_nanos, size).
fn file_key(metadata: &fs::Metadata) -> u64 {
    let mut hasher = SeaHasher::new();
    if let Ok(mtime) = metadata.modified() {
        if let Ok(duration) = mtime.duration_since(UNIX_EPOCH) {
            hasher.write_u64(duration.as_secs());
            hasher.write_u32(duration.subsec_nanos());
        }
    }
    hasher.write_u64(metadata.len());
    hasher.finish()
}

/// Hash stat results for workspace manifest files.
fn manifest_key(root: &Path) -> u64 {
    let manifests = [
        "go.mod",
        "Cargo.toml",
        "package.json",
        "tsconfig.json",
        "pyproject.toml",
    ];
    let mut hasher = SeaHasher::new();
    for name in &manifests {
        let path = root.join(name);
        if let Ok(meta) = fs::metadata(&path) {
            hasher.write_u64(file_key(&meta));
        }
    }
    hasher.finish()
}

/// Current time in millis since epoch.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Incremental build orchestrator
// ---------------------------------------------------------------------------

/// Result of an incremental build — pre-loaded data for index construction.
pub(crate) struct IncrementalBuildResult {
    /// Markdown file entries keyed by relative path.
    pub vault_files: BTreeMap<PathBuf, FileEntry>,
    /// Code imports keyed by relative path.
    pub code_imports: BTreeMap<PathBuf, Vec<ResolvedImport>>,
    /// Code symbols keyed by relative path.
    pub code_symbols: BTreeMap<PathBuf, Vec<Symbol>>,
    /// Pre-built workspace caches for symbol index construction.
    pub workspace_caches: WorkspaceCaches,
}

/// Run an incremental build: load cache, diff against disk, re-parse stale files, save.
///
/// When `progress` is `Some`, the bar's length is set to the total file count
/// and incremented once per processed file.
pub(crate) fn incremental_build(
    root: &Path,
    ignore_patterns: &[String],
    fresh: bool,
    progress: Option<&ProgressBar>,
) -> Result<IncrementalBuildResult> {
    // 1. Load cache (unless fresh).
    let mut cache = if fresh { None } else { IndexCache::load(root) };

    // 2. Check manifest_key — if changed, discard cache entirely.
    let current_manifest_key = manifest_key(root);
    if let Some(ref c) = cache {
        if c.manifest_key != current_manifest_key {
            cache = None;
        }
    }

    // 3. Build workspace caches (always — cheap, needed for re-parsing stale code).
    let workspace_caches = WorkspaceCaches::build(root, ignore_patterns)?;

    // 4. Discover all markdown + code files.
    let ignore_set = build_ignore_globset(ignore_patterns)?;
    let all_files = discover_files(root, root, &ignore_set, ALWAYS_IGNORED_DIRS)?;

    let md_files: Vec<PathBuf> = all_files
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        })
        .cloned()
        .collect();

    let code_files: Vec<PathBuf> = all_files
        .iter()
        .filter(|p| CodeLanguage::from_path(p).is_some())
        .cloned()
        .collect();

    let total_files = md_files.len() + code_files.len();
    if let Some(pb) = progress {
        pb.set_length(total_files as u64);
    }

    let now = now_millis();
    let cached_files = cache.as_ref().map(|c| &c.files);

    // 5a. Process markdown files.
    let md_results: Vec<_> = md_files
        .par_iter()
        .filter_map(|rel_path| {
            let abs_path = root.join(rel_path);
            let meta = fs::metadata(&abs_path).ok()?;
            let key = file_key(&meta);

            // Check cache.
            if let Some(cached) = cached_files.and_then(|m| m.get(rel_path)) {
                if cached.key == key {
                    if let CachedFileKind::Markdown {
                        ref headings,
                        ref links,
                    } = cached.kind
                    {
                        let entry = FileEntry {
                            rel_path: rel_path.clone(),
                            abs_path,
                            headings: headings.iter().map(to_heading).collect(),
                            links: links.iter().map(to_link).collect(),
                        };
                        if let Some(pb) = progress {
                            pb.inc(1);
                        }
                        return Some((rel_path.clone(), entry, key, true));
                    }
                }
            }

            // Cache miss — parse fresh.
            let source = fs::read_to_string(&abs_path).ok()?;
            let parsed = crate::index::parse_markdown(&source);
            let entry = FileEntry {
                rel_path: rel_path.clone(),
                abs_path,
                headings: parsed.headings,
                links: parsed.links,
            };
            if let Some(pb) = progress {
                pb.inc(1);
            }
            Some((rel_path.clone(), entry, key, false))
        })
        .collect();

    // 5b. Process code files.
    let code_results: Vec<_> = code_files
        .par_iter()
        .filter_map(|rel_path| {
            let language = CodeLanguage::from_path(rel_path)?;
            let abs_path = root.join(rel_path);
            let meta = fs::metadata(&abs_path).ok()?;
            let key = file_key(&meta);

            // Check cache.
            if let Some(cached) = cached_files.and_then(|m| m.get(rel_path)) {
                if cached.key == key {
                    if let CachedFileKind::Code {
                        ref symbols,
                        ref imports,
                    } = cached.kind
                    {
                        let syms: Vec<Symbol> = symbols.iter().map(to_symbol).collect();
                        let imps: Vec<ResolvedImport> = imports.iter().map(to_import).collect();
                        if let Some(pb) = progress {
                            pb.inc(1);
                        }
                        return Some((rel_path.clone(), syms, imps, key, true));
                    }
                }
            }

            // Cache miss — parse fresh.
            let abs_path = root.join(rel_path);
            let source = fs::read_to_string(&abs_path).ok()?;
            let symbols = extract_symbols(language, &source).ok()?;
            let imports =
                resolve_imports_for_language(root, rel_path, &source, language, &workspace_caches);
            if let Some(pb) = progress {
                pb.inc(1);
            }
            Some((rel_path.clone(), symbols, imports, key, false))
        })
        .collect();

    // 6. Assemble results and build new cache.
    let mut vault_files = BTreeMap::new();
    let mut code_imports = BTreeMap::new();
    let mut code_symbols = BTreeMap::new();
    let mut new_files: HashMap<PathBuf, CachedFileFacts> = HashMap::new();

    for (rel_path, entry, key, _was_cached) in md_results {
        let cached_facts = CachedFileFacts {
            key,
            last_seen: now,
            kind: CachedFileKind::Markdown {
                headings: entry.headings.iter().map(from_heading).collect(),
                links: entry.links.iter().map(from_link).collect(),
            },
        };
        new_files.insert(rel_path.clone(), cached_facts);
        vault_files.insert(rel_path, entry);
    }

    for (rel_path, symbols, imports, key, _was_cached) in code_results {
        let cached_facts = CachedFileFacts {
            key,
            last_seen: now,
            kind: CachedFileKind::Code {
                symbols: symbols.iter().map(from_symbol).collect(),
                imports: imports.iter().map(from_import).collect(),
            },
        };
        new_files.insert(rel_path.clone(), cached_facts);
        code_imports.insert(rel_path.clone(), imports);
        code_symbols.insert(rel_path, symbols);
    }

    // Merge old unseen entries for GC tracking.
    if let Some(old_cache) = cache {
        for (path, facts) in old_cache.files {
            new_files.entry(path).or_insert(facts);
        }
    }

    let mut new_cache = IndexCache {
        version: CACHE_VERSION.to_string(),
        manifest_key: current_manifest_key,
        files: new_files,
    };
    new_cache.gc();

    // 7. Save cache to disk.
    if let Err(err) = new_cache.save(root) {
        eprintln!("warning: failed to write index cache: {err:#}");
    }

    Ok(IncrementalBuildResult {
        vault_files,
        code_imports,
        code_symbols,
        workspace_caches,
    })
}

// ---------------------------------------------------------------------------
// Conversion: cached → live types
// ---------------------------------------------------------------------------

fn to_heading(c: &CachedHeading) -> Heading {
    Heading {
        title: c.title.clone(),
        anchor: c.anchor.clone(),
        level: c.level,
        line: c.line as usize,
        column: c.column as usize,
    }
}

fn to_link(c: &CachedLink) -> Link {
    Link {
        kind: if c.kind == 0 {
            LinkKind::Markdown
        } else {
            LinkKind::Wikilink
        },
        raw: c.raw.clone(),
        target: LinkTarget {
            file: c.target_file.clone(),
            anchor: c.target_anchor.clone(),
        },
        line: c.line as usize,
        column: c.column as usize,
    }
}

fn to_symbol(c: &CachedSymbol) -> Symbol {
    Symbol {
        name: c.name.clone(),
        parent: c.parent.clone(),
        kind: symbol_kind_from_u8(c.kind),
        display_kind: c.display_kind.clone(),
        line: c.line as usize,
        end_line: c.end_line as usize,
        start_byte: c.start_byte as usize,
        end_byte: c.end_byte as usize,
        is_public: c.is_public,
    }
}

fn to_import(c: &CachedImport) -> ResolvedImport {
    ResolvedImport {
        raw: c.raw.clone(),
        resolved_path: c.resolved_path.clone(),
        kind: import_kind_from_u8(c.kind),
        names: ImportNames {
            locals: c.names.locals.clone(),
            aliases: c.names.aliases.iter().cloned().collect(),
            is_namespace: c.names.is_namespace,
        },
        line: c.line as usize,
    }
}

// ---------------------------------------------------------------------------
// Conversion: live → cached types
// ---------------------------------------------------------------------------

fn from_heading(h: &Heading) -> CachedHeading {
    CachedHeading {
        title: h.title.clone(),
        anchor: h.anchor.clone(),
        level: h.level,
        line: h.line as u32,
        column: h.column as u32,
    }
}

fn from_link(l: &Link) -> CachedLink {
    CachedLink {
        kind: match l.kind {
            LinkKind::Markdown => 0,
            LinkKind::Wikilink => 1,
        },
        raw: l.raw.clone(),
        target_file: l.target.file.clone(),
        target_anchor: l.target.anchor.clone(),
        line: l.line as u32,
        column: l.column as u32,
    }
}

fn from_symbol(s: &Symbol) -> CachedSymbol {
    CachedSymbol {
        name: s.name.clone(),
        parent: s.parent.clone(),
        kind: symbol_kind_to_u8(s.kind),
        display_kind: s.display_kind.clone(),
        line: s.line as u32,
        end_line: s.end_line as u32,
        start_byte: s.start_byte as u32,
        end_byte: s.end_byte as u32,
        is_public: s.is_public,
    }
}

fn from_import(i: &ResolvedImport) -> CachedImport {
    CachedImport {
        raw: i.raw.clone(),
        resolved_path: i.resolved_path.clone(),
        kind: import_kind_to_u8(i.kind),
        names: CachedImportNames {
            locals: i.names.locals.clone(),
            aliases: i
                .names
                .aliases
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            is_namespace: i.names.is_namespace,
        },
        line: i.line as u32,
    }
}

// ---------------------------------------------------------------------------
// Enum discriminant helpers
// ---------------------------------------------------------------------------

fn symbol_kind_to_u8(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::Function => 0,
        SymbolKind::Method => 1,
        SymbolKind::Struct => 2,
        SymbolKind::Enum => 3,
        SymbolKind::Trait => 4,
        SymbolKind::TypeAlias => 5,
        SymbolKind::Class => 6,
        SymbolKind::Interface => 7,
        SymbolKind::Const => 8,
        SymbolKind::Static => 9,
        SymbolKind::Property => 10,
        SymbolKind::Getter => 11,
        SymbolKind::Setter => 12,
        SymbolKind::Module => 13,
        SymbolKind::Macro => 14,
        SymbolKind::Constructor => 15,
        SymbolKind::Variable => 16,
    }
}

fn symbol_kind_from_u8(v: u8) -> SymbolKind {
    match v {
        0 => SymbolKind::Function,
        1 => SymbolKind::Method,
        2 => SymbolKind::Struct,
        3 => SymbolKind::Enum,
        4 => SymbolKind::Trait,
        5 => SymbolKind::TypeAlias,
        6 => SymbolKind::Class,
        7 => SymbolKind::Interface,
        8 => SymbolKind::Const,
        9 => SymbolKind::Static,
        10 => SymbolKind::Property,
        11 => SymbolKind::Getter,
        12 => SymbolKind::Setter,
        13 => SymbolKind::Module,
        14 => SymbolKind::Macro,
        15 => SymbolKind::Constructor,
        16 => SymbolKind::Variable,
        _ => SymbolKind::Function, // fallback
    }
}

fn import_kind_to_u8(kind: ImportKind) -> u8 {
    match kind {
        ImportKind::Relative => 0,
        ImportKind::Workspace => 1,
        ImportKind::TsconfigPath => 2,
        ImportKind::External => 3,
    }
}

fn import_kind_from_u8(v: u8) -> ImportKind {
    match v {
        0 => ImportKind::Relative,
        1 => ImportKind::Workspace,
        2 => ImportKind::TsconfigPath,
        3 => ImportKind::External,
        _ => ImportKind::External, // fallback
    }
}
