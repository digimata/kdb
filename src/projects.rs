//! Projects table access and display.
//!
//! A project has a unique `slug` (stable short id used in commit scopes,
//! task IDs, and CLI args), a `path` relative to the kdb root, and a
//! lifecycle status (`active` | `paused` | `archived`).

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub path: String,
    pub status: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Project {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            path: row.get(3)?,
            status: row.get(4)?,
            description: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    }
}

const SELECT_COLS: &str =
    "id, slug, name, path, status, description, created_at, updated_at";

/// List projects, ordered by slug. Archived projects are excluded unless
/// `include_archived` is set.
pub fn list(conn: &Connection, include_archived: bool) -> Result<Vec<Project>> {
    let sql = if include_archived {
        format!("SELECT {SELECT_COLS} FROM projects ORDER BY slug")
    } else {
        format!(
            "SELECT {SELECT_COLS} FROM projects \
             WHERE status != 'archived' ORDER BY slug"
        )
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], Project::from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read projects")
}

/// Fetch a project by slug. Returns `None` if no match.
pub fn get_by_slug(conn: &Connection, slug: &str) -> Result<Option<Project>> {
    let sql = format!("SELECT {SELECT_COLS} FROM projects WHERE slug = ?");
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_row([slug], Project::from_row)
        .optional()
        .context("failed to query project")
}

pub struct AddArgs<'a> {
    pub slug: &'a str,
    pub name: Option<&'a str>,
    pub path: &'a str,
    pub description: Option<&'a str>,
}

/// Insert a new project. Fails if slug or path is already taken.
pub fn add(conn: &Connection, args: AddArgs) -> Result<Project> {
    let name = args.name.unwrap_or(args.slug);
    conn.execute(
        "INSERT INTO projects (slug, name, path, description) \
         VALUES (?, ?, ?, ?)",
        params![args.slug, name, args.path, args.description],
    )
    .with_context(|| format!("failed to insert project {}", args.slug))?;
    get_by_slug(conn, args.slug)?
        .with_context(|| format!("project {} missing after insert", args.slug))
}

#[derive(Default)]
pub struct EditArgs<'a> {
    pub name: Option<&'a str>,
    pub path: Option<&'a str>,
    pub status: Option<&'a str>,
    pub description: Option<&'a str>,
}

impl EditArgs<'_> {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.path.is_none()
            && self.status.is_none()
            && self.description.is_none()
    }
}

/// Update mutable fields on a project. `None` fields are left unchanged.
pub fn edit(conn: &Connection, slug: &str, args: EditArgs) -> Result<Project> {
    if args.is_empty() {
        bail!("no fields to update");
    }
    if get_by_slug(conn, slug)?.is_none() {
        bail!("project not found: {slug}");
    }
    conn.execute(
        "UPDATE projects SET \
            name        = COALESCE(?, name), \
            path        = COALESCE(?, path), \
            status      = COALESCE(?, status), \
            description = COALESCE(?, description), \
            updated_at  = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE slug = ?",
        params![args.name, args.path, args.status, args.description, slug],
    )
    .with_context(|| format!("failed to update project {slug}"))?;
    get_by_slug(conn, slug)?
        .with_context(|| format!("project {slug} missing after update"))
}

/// Return the registered project whose `path` is the deepest prefix of
/// `cwd` relative to the kdb `root`. Returns `None` if no project matches.
pub fn resolve_active(
    conn: &Connection,
    root: &Path,
    cwd: &Path,
) -> Result<Option<Project>> {
    let rel = match cwd.strip_prefix(root) {
        Ok(r) => r.to_path_buf(),
        Err(_) => return Ok(None),
    };
    let all = list(conn, true)?;
    let mut best: Option<Project> = None;
    for p in all {
        let proj_path = Path::new(&p.path);
        if !rel.starts_with(proj_path) {
            continue;
        }
        let depth = proj_path.components().count();
        match &best {
            Some(cur) if Path::new(&cur.path).components().count() >= depth => {}
            _ => best = Some(p),
        }
    }
    Ok(best)
}

/// Render a list of projects as an aligned text table.
pub fn render_list(projects: &[Project]) -> String {
    if projects.is_empty() {
        return String::from("(no projects)\n");
    }
    let slug_w = projects.iter().map(|p| p.slug.len()).max().unwrap_or(4).max(4);
    let name_w = projects.iter().map(|p| p.name.len()).max().unwrap_or(4).max(4);
    let status_w = projects
        .iter()
        .map(|p| p.status.len())
        .max()
        .unwrap_or(6)
        .max(6);

    let mut out = String::new();
    out.push_str(&format!(
        "{:<slug_w$}  {:<name_w$}  {:<status_w$}  path\n",
        "slug", "name", "status",
    ));
    for p in projects {
        out.push_str(&format!(
            "{:<slug_w$}  {:<name_w$}  {:<status_w$}  {}\n",
            p.slug, p.name, p.status, p.path,
        ));
    }
    out
}

/// Render a single project as a human-readable block.
pub fn render_show(p: &Project) -> String {
    let mut out = String::new();
    out.push_str(&format!("slug:        {}\n", p.slug));
    out.push_str(&format!("name:        {}\n", p.name));
    out.push_str(&format!("path:        {}\n", p.path));
    out.push_str(&format!("status:      {}\n", p.status));
    if let Some(desc) = &p.description {
        out.push_str(&format!("description: {desc}\n"));
    }
    out.push_str(&format!("created_at:  {}\n", p.created_at));
    out.push_str(&format!("updated_at:  {}\n", p.updated_at));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::project::root::ROOT_MARKER;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Connection) {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(ROOT_MARKER)).unwrap();
        let conn = db::open(tmp.path()).unwrap();
        (tmp, conn)
    }

    #[test]
    fn add_then_list_and_show() {
        let (_tmp, conn) = setup();
        let p = add(
            &conn,
            AddArgs {
                slug: "kdb",
                name: Some("kdb"),
                path: "projects/kdb",
                description: Some("knowledge db"),
            },
        )
        .unwrap();
        assert_eq!(p.slug, "kdb");

        let all = list(&conn, false).unwrap();
        assert_eq!(all.len(), 1);

        let got = get_by_slug(&conn, "kdb").unwrap().unwrap();
        assert_eq!(got.name, "kdb");
        assert_eq!(got.description.as_deref(), Some("knowledge db"));
    }

    #[test]
    fn duplicate_slug_errors() {
        let (_tmp, conn) = setup();
        add(
            &conn,
            AddArgs {
                slug: "kdb",
                name: None,
                path: "projects/kdb",
                description: None,
            },
        )
        .unwrap();
        let err = add(
            &conn,
            AddArgs {
                slug: "kdb",
                name: None,
                path: "elsewhere",
                description: None,
            },
        );
        assert!(err.is_err());
    }

    #[test]
    fn edit_updates_fields_and_timestamp() {
        let (_tmp, conn) = setup();
        let before = add(
            &conn,
            AddArgs {
                slug: "kdb",
                name: None,
                path: "projects/kdb",
                description: None,
            },
        )
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(5));
        let after = edit(
            &conn,
            "kdb",
            EditArgs {
                name: Some("KDB"),
                status: Some("paused"),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(after.name, "KDB");
        assert_eq!(after.status, "paused");
        assert_eq!(after.path, before.path);
        assert_ne!(after.updated_at, before.updated_at);
    }

    #[test]
    fn edit_rejects_empty() {
        let (_tmp, conn) = setup();
        add(
            &conn,
            AddArgs {
                slug: "kdb",
                name: None,
                path: "projects/kdb",
                description: None,
            },
        )
        .unwrap();
        assert!(edit(&conn, "kdb", EditArgs::default()).is_err());
    }

    #[test]
    fn list_filters_archived_by_default() {
        let (_tmp, conn) = setup();
        add(
            &conn,
            AddArgs {
                slug: "a",
                name: None,
                path: "a",
                description: None,
            },
        )
        .unwrap();
        add(
            &conn,
            AddArgs {
                slug: "b",
                name: None,
                path: "b",
                description: None,
            },
        )
        .unwrap();
        edit(
            &conn,
            "b",
            EditArgs {
                status: Some("archived"),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(list(&conn, false).unwrap().len(), 1);
        assert_eq!(list(&conn, true).unwrap().len(), 2);
    }
}
