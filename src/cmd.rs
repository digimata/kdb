//! CLI command implementations.
//!
//! Each public function corresponds to a subcommand of the `kdb` binary:
//! `init`, `check`, `outline`, `tree`, `symbols`, `refs`, `deps`, `graph`, `fmt`, and `lsp`.

use anyhow::{Context, Result, bail};
use serde_json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cycles;
use crate::db;
use crate::fmt;
use crate::index::{self, VaultIndex, WorkspaceIndex, deps as md_deps, refs};
use crate::labels;
use crate::lang::CodeLanguage;
use crate::materialize;
use crate::projects;
use crate::render;
use crate::symbols;
use crate::tasks;
use crate::tree;
use crate::update;
use crate::workspace::{self, WorkspaceContext};

use rusqlite::Connection;

// -----------------------------------------
// projects/kdb/src/cmd.rs
//
// pub struct CmdContext                 L74
//   pub fn from_path()                  L85
//   pub fn build_index()                L95
//   pub fn build_workspace_index()     L100
//   pub fn rel_path()                  L108
// pub fn init()                        L135
// pub fn check()                       L192
// pub fn tree()                        L209
// pub fn symbols()                     L257
// pub fn refs()                        L318
// pub fn deps()                        L389
// pub fn graph()                       L424
// pub fn render()                      L439
// pub fn format()                      L489
// pub fn update()                      L530
// pub fn projects_list()               L536
// pub fn projects_add()                L552
// pub fn projects_edit()               L579
// pub fn projects_show()               L605
// fn resolve_project()                 L624
// fn resolve_cycle()                   L642
// fn resolve_parent()                  L656
// fn parse_statuses()                  L669
// pub fn tasks_list()                  L690
// pub fn tasks_add()                   L731
// pub fn tasks_edit()                  L765
// pub fn tasks_show()                  L797
// struct TaskShowOutput                L807
// pub fn cycles_list()                 L825
// pub fn cycles_add()                  L839
// pub fn cycles_edit()                 L867
// pub fn cycles_show()                 L892
// pub fn labels_list()                 L908
// pub fn labels_add()                  L922
// pub fn labels_edit()                 L937
// pub fn labels_show()                 L952
// pub fn tasks_label_add()             L969
// pub fn tasks_label_rm()              L988
// pub fn tasks_set_status()           L1007
// -----------------------------------------

/// CLI command context: resolved start path + workspace state.
pub struct CmdContext {
    /// Resolved absolute start path (from CLI arg or cwd).
    pub start: PathBuf,
    /// Discovered workspace context (root, ignore patterns, ignore set).
    pub workspace: WorkspaceContext,
}

impl CmdContext {
    /// Resolve a start path and discover the workspace root.
    ///
    /// When `path` is `None`, falls back to the current working directory.
    pub fn from_path(path: Option<&Path>) -> Result<Self> {
        let start = match path {
            Some(p) => workspace::root::make_absolute(p)?,
            None => env::current_dir().context("failed to read current directory")?,
        };
        let workspace = WorkspaceContext::discover(&start)?;
        Ok(Self { start, workspace })
    }

    /// Build a [`VaultIndex`] (markdown only) using the workspace's ignore patterns.
    pub fn build_index(&self) -> Result<VaultIndex> {
        VaultIndex::build_with_ignores(&self.workspace.root, &self.workspace.ignore_patterns)
    }

    /// Build a [`WorkspaceIndex`] (vault + code) using the workspace's ignore patterns.
    pub fn build_workspace_index(&self) -> Result<WorkspaceIndex> {
        WorkspaceIndex::build_with_ignores(&self.workspace.root, &self.workspace.ignore_patterns)
    }

    /// Canonicalize an absolute path and return its root-relative form.
    ///
    /// Performs `canonicalize` → `strip_prefix(root)` → `normalize_rel_path`,
    /// producing a clean relative path suitable for index lookups.
    pub fn rel_path(&self, abs: &Path) -> Result<PathBuf> {
        let canonical = abs
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", abs.display()))?;
        let root = &self.workspace.root;
        canonical
            .strip_prefix(root)
            .with_context(|| {
                format!(
                    "path {} is not inside kdb root {}",
                    canonical.display(),
                    root.display()
                )
            })
            .and_then(|rel| {
                workspace::paths::normalize_rel_path(rel).with_context(|| {
                    format!(
                        "path {} resolves outside kdb root {}",
                        canonical.display(),
                        root.display()
                    )
                })
            })
    }
}

/// Initialize a kdb workspace by creating `.kdb/config.toml`.
pub fn init(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        Some(path) => workspace::root::make_absolute(&path)?,
        None => env::current_dir().context("failed to read current directory")?,
    };

    if !start.exists() {
        bail!("path does not exist: {}", start.display());
    }

    if !start.is_dir() {
        bail!("init path must be a directory: {}", start.display());
    }

    let root = start
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", start.display()))?;

    let marker_dir = root.join(workspace::root::ROOT_MARKER);
    if marker_dir.exists() {
        bail!(
            "{} already exists in {}",
            workspace::root::ROOT_MARKER,
            root.display()
        );
    }

    fs::create_dir_all(&marker_dir)
        .with_context(|| format!("failed to create {}", marker_dir.display()))?;

    let workspace_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kdb")
        .replace('"', "\\\"");

    let config = workspace::root::config_path(&root);
    let default_config = format!("[workspace]\nname = \"{workspace_name}\"\n");
    fs::write(&config, default_config)
        .with_context(|| format!("failed to write {}", config.display()))?;

    let ignore_path = marker_dir.join("ignore");
    fs::write(&ignore_path, workspace::ignore::DEFAULT_IGNORE)
        .with_context(|| format!("failed to write {}", ignore_path.display()))?;

    db::open(&root)
        .with_context(|| format!("failed to initialize {}", db::db_path(&root).display()))?;

    println!("initialized kdb workspace at {}", root.display());

    Ok(())
}

/// Validate all links in the vault and report broken references and orphan files.
///
/// Returns `Ok(true)` if any issues were found (caller should exit with code 1),
/// or `Ok(false)` if the vault is clean.
pub fn check(path: Option<PathBuf>, list_orphans: bool) -> Result<bool> {
    let has_scope = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref())?;
    let index = ctx.build_index()?;
    let mut report = index.check();

    if has_scope {
        let scope_rel = ctx.rel_path(&ctx.start)?;
        let scope_is_dir = ctx.start.is_dir();
        report = report.scoped_to(&scope_rel, scope_is_dir);
    }

    report.print(list_orphans);
    Ok(report.has_errors())
}

/// Print a filtered workspace tree for a path under the current kdb root.
pub fn tree(
    path: Option<PathBuf>,
    level: Option<usize>,
    as_json: bool,
    all: bool,
    dirs_only: bool,
    full_path: bool,
    ignore: Vec<String>,
    pattern: Vec<String>,
) -> Result<()> {
    let has_explicit_path = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref())?;

    if !ctx.start.exists() {
        bail!("path does not exist: {}", ctx.start.display());
    }

    let tree_start = if has_explicit_path {
        ctx.start.clone()
    } else {
        ctx.workspace.root.clone()
    };

    let tree = tree::build_tree(
        &ctx.workspace.root,
        &tree_start,
        &ctx.workspace.ignore_patterns,
        tree::TreeOptions {
            max_depth: level,
            show_hidden: all,
            dirs_only,
            full_paths: full_path,
            ignore_patterns: ignore,
            include_patterns: pattern,
        },
    )?;
    if as_json {
        let output =
            serde_json::to_string_pretty(&tree).context("failed to serialize tree as JSON")?;
        println!("{output}");
    } else {
        println!("{}", tree::render_text(&tree));
    }

    Ok(())
}

/// Print symbols for one or more files and/or directories.
pub fn symbols(
    paths: Vec<PathBuf>,
    selectors: Vec<String>,
    as_json: bool,
    public_only: bool,
) -> Result<()> {
    assert!(!paths.is_empty(), "at least one path is required");

    let ctx = CmdContext::from_path(Some(&paths[0]))?;
    let files = symbols::query::expand_paths(&ctx.workspace, &paths)?;
    assert!(!files.is_empty(), "no supported files found in given paths");

    let multi = files.len() > 1;
    if multi && !selectors.is_empty() {
        bail!(
            "-s/--symbol requires a single definition file, got {} files",
            files.len()
        );
    }

    if selectors.is_empty() {
        let mut all_rows: Vec<(PathBuf, Vec<symbols::display::SymbolRow>)> = Vec::new();
        for (abs, rel) in &files {
            let mut rows = symbols::query::collect_rows(&ctx.workspace.root, abs, rel)?;
            if public_only {
                rows.retain(|row| row.is_public);
            }
            all_rows.push((rel.clone(), rows));
        }

        if as_json {
            let flat: Vec<_> = all_rows.iter().flat_map(|(_, rows)| rows).collect();
            let output = serde_json::to_string_pretty(&flat)
                .context("failed to serialize symbols as JSON")?;
            println!("{output}");
        } else if multi {
            symbols::display::print_multi_text(&all_rows);
        } else {
            symbols::display::print_text(&all_rows[0].1);
        }
    } else {
        let selector_strs: Vec<&str> = selectors.iter().map(String::as_str).collect();
        let mut all_rows = Vec::new();
        for (abs, rel) in &files {
            let rows = symbols::query::collect_body_rows(abs, rel, &selector_strs, public_only)?;
            all_rows.extend(rows);
        }

        if as_json {
            let output = serde_json::to_string_pretty(&all_rows)
                .context("failed to serialize symbol bodies as JSON")?;
            println!("{output}");
        } else {
            symbols::display::print_bodies_text(&all_rows);
        }
    }

    Ok(())
}

/// Find inbound markdown references or code symbol references.
pub fn refs(
    target: String,
    symbol: Option<String>,
    context_lines: Option<usize>,
    as_json: bool,
    count_only: bool,
    files_only: bool,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;

    if let Some(symbol_name) = symbol {
        let index = WorkspaceIndex::build_for_target(
            &ctx.workspace.root,
            &ctx.workspace.ignore_patterns,
            &target,
        )?;
        let inbound =
            refs::collect_symbol_refs(&index.code, &ctx.workspace.root, &target, &symbol_name)?;

        if count_only {
            println!("{}", inbound.len());
            return Ok(());
        }

        if files_only {
            refs::print_symbol_refs_files(&inbound);
            return Ok(());
        }

        if as_json {
            let output = serde_json::to_string_pretty(&inbound)
                .context("failed to serialize symbol refs as JSON")?;
            println!("{output}");
        } else {
            let options = refs::SymbolRefRenderOptions::new(context_lines.unwrap_or(0));
            refs::print_symbol_refs_text(&ctx.workspace.root, &inbound, options)?;
        }

        return Ok(());
    }

    if context_lines.is_some() {
        bail!("--context is currently supported only with --symbol");
    }

    let target = index::refs::parse_target(&target)?;
    let index = ctx.build_index()?;
    let inbound = refs::collect_inbound(&index, &ctx.workspace.root, target)?;

    if count_only {
        println!("{}", inbound.len());
        return Ok(());
    }

    if files_only {
        refs::print_files(&inbound);
        return Ok(());
    }

    if as_json {
        let output =
            serde_json::to_string_pretty(&inbound).context("failed to serialize refs as JSON")?;
        println!("{output}");
    } else {
        refs::print_text(&inbound);
    }

    Ok(())
}

/// List outbound dependencies for a markdown or supported code file.
pub fn deps(target: String, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let source_file = index::resolve_file_target(&ctx.workspace.root, &target)?;
    let is_markdown = source_file
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));

    if !is_markdown && CodeLanguage::from_path(&source_file).is_none() {
        bail!(
            "deps is not supported for file type: {}",
            source_file.display()
        );
    }

    let pi = ctx.build_workspace_index()?;

    let outbound = if is_markdown {
        md_deps::collect_outbound(&pi.vault, &source_file)?
    } else {
        md_deps::collect_code_outbound(&pi.code, &source_file)?
    };

    if as_json {
        let output =
            serde_json::to_string_pretty(&outbound).context("failed to serialize deps as JSON")?;
        println!("{output}");
    } else {
        md_deps::print_text(&outbound);
    }

    Ok(())
}

/// Stub for `kdb graph` until graph rendering lands.
pub fn graph(path: Option<PathBuf>) -> Result<()> {
    let requested = path
        .as_ref()
        .map(|value| value.display().to_string())
        .unwrap_or_else(|| "<root>".to_string());
    bail!(
        "`kdb graph` is not implemented yet (path: {requested}). See .issues/iss-0021-graph-command.md"
    )
}

/// Resolve markdown includes, or materialize per-project task files.
///
/// With `file`: resolve `![[]]` embeds to stdout. With `--project` or
/// `--all`: write materialized `index.md` + per-task files for the
/// matching projects.
pub fn render(
    file: Option<PathBuf>,
    project: Option<String>,
    all: bool,
    limit: Option<i64>,
) -> Result<()> {
    if all || project.is_some() {
        if file.is_some() {
            bail!("--project/--all cannot be combined with a file argument");
        }
        let ctx = CmdContext::from_path(None)?;
        let conn = db::open(&ctx.workspace.root)?;
        let written = if all {
            materialize::materialize_all(&conn, &ctx.workspace.root, limit)?
        } else {
            let slug = project.expect("project checked above");
            vec![materialize::materialize_project(
                &conn,
                &ctx.workspace.root,
                &slug,
                limit,
            )?]
        };
        for p in &written {
            println!("wrote {}", p.display());
        }
        return Ok(());
    }
    if limit.is_some() {
        bail!("--limit only applies with --project/--all");
    }

    let file = file.context(
        "missing file argument — pass a markdown file, \
         --project <slug>, or --all",
    )?;
    let ctx = CmdContext::from_path(Some(&file))?;
    let rel_path = ctx.rel_path(&ctx.start)?;

    let output = render::render_file(&ctx.workspace.root, &rel_path)
        .with_context(|| format!("failed to render {}", rel_path.display()))?;

    print!("{output}");
    Ok(())
}

/// Generate or update code index headers for supported code files.
///
/// Walks the workspace root and rewrites Rust, TypeScript/JavaScript, Python,
/// and Go files with a managed index block at the top of each file.
pub fn format(path: Option<PathBuf>, force: bool) -> Result<()> {
    let has_explicit_path = path.is_some();
    let ctx = CmdContext::from_path(path.as_deref())?;

    if !ctx.start.exists() {
        bail!("path does not exist: {}", ctx.start.display());
    }

    let fmt_target = if has_explicit_path {
        ctx.start
    } else {
        ctx.workspace.root.clone()
    };
    let report = fmt::format_path(
        &ctx.workspace.root,
        &fmt_target,
        &ctx.workspace.ignore_patterns,
        force,
    )?;
    println!(
        "kdb fmt: updated {} of {} files",
        report.updated_files, report.scanned_files
    );

    if !report.warnings.is_empty() {
        eprintln!("kdb fmt: {} warning(s)", report.warnings.len());
        for warning in &report.warnings {
            eprintln!(
                "warning: {} ({})",
                warning.message,
                warning.rel_path.display()
            );
        }
    }

    Ok(())
}

/// Check for updates and optionally self-update the binary.
///
/// When `check_only` is true, prints version info without replacing the binary.
pub fn update(check_only: bool) -> Result<()> {
    let updater = update::Updater::new();
    updater.run(check_only)
}

/// List projects. Archived projects are hidden unless `include_archived`.
pub fn projects_list(include_archived: bool, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let rows = projects::list(&conn, include_archived)?;

    if as_json {
        let output =
            serde_json::to_string_pretty(&rows).context("failed to serialize projects as JSON")?;
        println!("{output}");
    } else {
        print!("{}", projects::render_list(&rows));
    }
    Ok(())
}

/// Insert a new project.
pub fn projects_add(
    slug: String,
    alias: String,
    path: String,
    name: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let created = projects::add(
        &conn,
        projects::AddArgs {
            slug: &slug,
            alias: &alias,
            name: name.as_deref(),
            path: &path,
            description: description.as_deref(),
        },
    )?;
    println!(
        "added project {} [{}] ({})",
        created.slug, created.alias, created.path
    );
    Ok(())
}

/// Update mutable fields on an existing project.
pub fn projects_edit(
    slug: String,
    alias: Option<String>,
    name: Option<String>,
    path: Option<String>,
    status: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let updated = projects::edit(
        &conn,
        &slug,
        projects::EditArgs {
            alias: alias.as_deref(),
            name: name.as_deref(),
            path: path.as_deref(),
            status: status.as_deref(),
            description: description.as_deref(),
        },
    )?;
    println!("updated project {}", updated.slug);
    Ok(())
}

/// Show a single project.
pub fn projects_show(slug: String, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let project = projects::get_by_slug(&conn, &slug)?
        .with_context(|| format!("project not found: {slug}"))?;

    if as_json {
        let output = serde_json::to_string_pretty(&project)
            .context("failed to serialize project as JSON")?;
        println!("{output}");
    } else {
        print!("{}", projects::render_show(&project));
    }
    Ok(())
}

/// Resolve the project to operate on. Prefers the explicit `--project`
/// flag; otherwise falls back to the project registered at the current
/// working directory.
fn resolve_project(
    conn: &Connection,
    root: &Path,
    explicit: Option<&str>,
) -> Result<projects::Project> {
    if let Some(slug) = explicit {
        return projects::get_by_slug(conn, slug)?
            .with_context(|| format!("project not found: {slug}"));
    }
    let cwd = env::current_dir().context("failed to read current directory")?;
    let cwd = cwd.canonicalize().unwrap_or(cwd);
    projects::resolve_active(conn, root, &cwd)?.context(
        "no project for current directory — pass -P/--project or \
         register one with `kdb projects add`",
    )
}

/// Resolve an optional cycle key to its numeric id.
fn resolve_cycle(conn: &Connection, key: Option<&str>) -> Result<Option<Option<i64>>> {
    match key {
        None => Ok(None),
        Some(k) if k.is_empty() => Ok(Some(None)),
        Some(k) => {
            let id: i64 = conn
                .query_row("SELECT id FROM cycles WHERE key = ?", [k], |row| row.get(0))
                .with_context(|| format!("cycle not found: {k}"))?;
            Ok(Some(Some(id)))
        }
    }
}

/// Resolve an optional parent task id (external form `slug-seq`).
fn resolve_parent(conn: &Connection, id: Option<&str>) -> Result<Option<Option<i64>>> {
    match id {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(Some(None)),
        Some(s) => {
            let parsed = tasks::TaskId::parse(s)?;
            let view = tasks::get(conn, &parsed)?
                .with_context(|| format!("parent task not found: {s}"))?;
            Ok(Some(Some(view.task.id)))
        }
    }
}

fn parse_statuses(s: &str) -> Result<Option<Vec<String>>> {
    if s == "all" {
        return Ok(None);
    }
    let parts: Vec<String> = s
        .split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    for p in &parts {
        if !tasks::STATUSES.contains(&p.as_str()) {
            bail!(
                "invalid status '{p}' (expected {} or 'all')",
                tasks::STATUSES.join(", ")
            );
        }
    }
    Ok(Some(parts))
}

/// List tasks.
pub fn tasks_list(
    status: String,
    project: Option<String>,
    cycle: Option<String>,
    priority: Option<i64>,
    limit: Option<i64>,
    as_json: bool,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;

    let statuses_owned = parse_statuses(&status)?;
    let statuses_refs: Option<Vec<&str>> = statuses_owned
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect());

    let project_slug = match project.as_deref() {
        Some("all") | Some("") | None => None,
        Some(s) => Some(s.to_string()),
    };

    let filters = tasks::ListFilters {
        statuses: statuses_refs.as_deref(),
        project_slug: project_slug.as_deref(),
        cycle_key: cycle.as_deref(),
        priority,
        limit,
    };
    let rows = tasks::list(&conn, filters)?;

    if as_json {
        let output =
            serde_json::to_string_pretty(&rows).context("failed to serialize tasks as JSON")?;
        println!("{output}");
    } else {
        print!("{}", tasks::render_list(&rows));
    }
    Ok(())
}

/// Add a new task.
pub fn tasks_add(
    title: String,
    project: Option<String>,
    body: Option<String>,
    priority: Option<i64>,
    cycle: Option<String>,
    parent: Option<String>,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let mut conn = db::open(&ctx.workspace.root)?;
    let proj = resolve_project(&conn, &ctx.workspace.root, project.as_deref())?;

    let cycle_id = resolve_cycle(&conn, cycle.as_deref())?.flatten();
    let parent_id = resolve_parent(&conn, parent.as_deref())?.flatten();

    let view = tasks::add(
        &mut conn,
        tasks::AddArgs {
            project_id: proj.id,
            title: &title,
            body: body.as_deref(),
            priority,
            cycle_id,
            parent_id,
            seq: None,
            status: None,
        },
    )?;
    materialize::materialize_project(&conn, &ctx.workspace.root, &view.project_slug, None)?;
    println!("added task {}", view.external_id());
    Ok(())
}

/// Edit an existing task.
pub fn tasks_edit(
    id: String,
    title: Option<String>,
    body: Option<String>,
    priority: Option<i64>,
    cycle: Option<String>,
    parent: Option<String>,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let parsed = tasks::TaskId::parse(&id)?;

    let cycle_update = resolve_cycle(&conn, cycle.as_deref())?;
    let parent_update = resolve_parent(&conn, parent.as_deref())?;

    let view = tasks::edit(
        &conn,
        &parsed,
        tasks::EditArgs {
            title: title.as_deref(),
            body: body.as_deref(),
            priority,
            cycle_id: cycle_update,
            parent_id: parent_update,
        },
    )?;
    materialize::materialize_project(&conn, &ctx.workspace.root, &view.project_slug, None)?;
    println!("updated task {}", view.external_id());
    Ok(())
}

/// Show a single task.
pub fn tasks_show(id: String, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let parsed = tasks::TaskId::parse(&id)?;
    let view = tasks::get(&conn, &parsed)?.with_context(|| format!("task not found: {id}"))?;
    let task_labels = labels::for_task(&conn, view.task.id)?;
    let slugs: Vec<&str> = task_labels.iter().map(|l| l.slug.as_str()).collect();

    if as_json {
        #[derive(serde::Serialize)]
        struct TaskShowOutput<'a> {
            #[serde(flatten)]
            task: &'a tasks::TaskView,
            labels: &'a [labels::Label],
        }
        let output = serde_json::to_string_pretty(&TaskShowOutput {
            task: &view,
            labels: &task_labels,
        })
        .context("failed to serialize task as JSON")?;
        println!("{output}");
    } else {
        print!("{}", tasks::render_show(&view, &slugs));
    }
    Ok(())
}

/// List cycles.
pub fn cycles_list(as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let rows = cycles::list(&conn)?;
    if as_json {
        let output =
            serde_json::to_string_pretty(&rows).context("failed to serialize cycles as JSON")?;
        println!("{output}");
    } else {
        print!("{}", cycles::render_list(&rows));
    }
    Ok(())
}

pub fn cycles_add(
    key: String,
    start: String,
    end: String,
    description: Option<String>,
    status: Option<String>,
    path: Option<String>,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let created = cycles::add(
        &conn,
        cycles::AddArgs {
            key: &key,
            start_date: &start,
            end_date: &end,
            description: description.as_deref(),
            status: status.as_deref(),
            path: path.as_deref(),
        },
    )?;
    println!(
        "added cycle {} ({} → {})",
        created.key, created.start_date, created.end_date
    );
    Ok(())
}

pub fn cycles_edit(
    key: String,
    start: Option<String>,
    end: Option<String>,
    description: Option<String>,
    status: Option<String>,
    path: Option<String>,
) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let updated = cycles::edit(
        &conn,
        &key,
        cycles::EditArgs {
            start_date: start.as_deref(),
            end_date: end.as_deref(),
            description: description.as_deref(),
            status: status.as_deref(),
            path: path.as_deref(),
        },
    )?;
    println!("updated cycle {}", updated.key);
    Ok(())
}

pub fn cycles_show(key: String, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let cycle =
        cycles::get_by_key(&conn, &key)?.with_context(|| format!("cycle not found: {key}"))?;
    if as_json {
        let output =
            serde_json::to_string_pretty(&cycle).context("failed to serialize cycle as JSON")?;
        println!("{output}");
    } else {
        print!("{}", cycles::render_show(&cycle));
    }
    Ok(())
}

/// List labels.
pub fn labels_list(as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let rows = labels::list(&conn)?;
    if as_json {
        let output =
            serde_json::to_string_pretty(&rows).context("failed to serialize labels as JSON")?;
        println!("{output}");
    } else {
        print!("{}", labels::render_list(&rows));
    }
    Ok(())
}

pub fn labels_add(slug: String, name: Option<String>, color: Option<String>) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let created = labels::add(
        &conn,
        labels::AddArgs {
            slug: &slug,
            name: name.as_deref(),
            color: color.as_deref(),
        },
    )?;
    println!("added label {}", created.slug);
    Ok(())
}

pub fn labels_edit(slug: String, name: Option<String>, color: Option<String>) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let updated = labels::edit(
        &conn,
        &slug,
        labels::EditArgs {
            name: name.as_deref(),
            color: color.as_deref(),
        },
    )?;
    println!("updated label {}", updated.slug);
    Ok(())
}

pub fn labels_show(slug: String, as_json: bool) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let label =
        labels::get_by_slug(&conn, &slug)?.with_context(|| format!("label not found: {slug}"))?;
    if as_json {
        let output =
            serde_json::to_string_pretty(&label).context("failed to serialize label as JSON")?;
        println!("{output}");
    } else {
        print!("{}", labels::render_show(&label));
    }
    Ok(())
}

/// Attach one or more labels to a task. Unknown label slugs are
/// created on the fly.
pub fn tasks_label_add(id: String, label_slugs: Vec<String>) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let parsed = tasks::TaskId::parse(&id)?;
    let view = tasks::get(&conn, &parsed)?.with_context(|| format!("task not found: {id}"))?;
    for slug in &label_slugs {
        let label = labels::upsert_by_slug(&conn, slug)?;
        labels::attach(&conn, view.task.id, label.id)?;
    }
    materialize::materialize_project(&conn, &ctx.workspace.root, &view.project_slug, None)?;
    println!(
        "attached {} label(s) to {}",
        label_slugs.len(),
        view.external_id()
    );
    Ok(())
}

/// Detach one or more labels from a task.
pub fn tasks_label_rm(id: String, label_slugs: Vec<String>) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let parsed = tasks::TaskId::parse(&id)?;
    let view = tasks::get(&conn, &parsed)?.with_context(|| format!("task not found: {id}"))?;
    let mut removed = 0usize;
    for slug in &label_slugs {
        if let Some(label) = labels::get_by_slug(&conn, slug)? {
            if labels::detach(&conn, view.task.id, label.id)? {
                removed += 1;
            }
        }
    }
    materialize::materialize_project(&conn, &ctx.workspace.root, &view.project_slug, None)?;
    println!("detached {removed} label(s) from {}", view.external_id());
    Ok(())
}

/// Transition a task to a new status.
pub fn tasks_set_status(id: String, status: &str) -> Result<()> {
    let ctx = CmdContext::from_path(None)?;
    let conn = db::open(&ctx.workspace.root)?;
    let parsed = tasks::TaskId::parse(&id)?;
    let view = tasks::set_status(&conn, &parsed, status)?;
    materialize::materialize_project(&conn, &ctx.workspace.root, &view.project_slug, None)?;
    println!("{} -> {}", view.external_id(), view.task.status);
    Ok(())
}
