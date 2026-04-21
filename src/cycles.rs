//! Cycles table access and display.
//!
//! A cycle is a time-boxed unit of work (typically a week) identified
//! by a short `key` like `C-15`. Tasks can be scoped to a cycle via
//! `tasks.cycle_id`.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

// ---------------------------------
// projects/kdb/src/cycles.rs
//
// pub const STATUSES            L33
// pub struct Cycle              L36
//   fn from_row()               L48
// const SELECT_COLS             L62
// pub fn list()                 L65
// pub fn get_by_key()           L74
// pub struct AddArgs            L82
// pub fn add()                  L92
// pub struct EditArgs          L117
//   fn is_empty()              L126
// pub fn edit()                L135
// pub fn render_list()         L168
// pub fn render_show()         L198
// mod tests                    L215
// fn setup()                   L221
// fn add_list_edit()           L229
// fn duplicate_key_errors()    L276
// ---------------------------------

pub const STATUSES: &[&str] = &["planned", "active", "done", "abandoned"];

#[derive(Debug, Clone, Serialize)]
pub struct Cycle {
    pub id: i64,
    pub key: String,
    pub start_date: String,
    pub end_date: String,
    pub description: Option<String>,
    pub status: String,
    pub path: Option<String>,
    pub created_at: String,
}

impl Cycle {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            key: row.get(1)?,
            start_date: row.get(2)?,
            end_date: row.get(3)?,
            description: row.get(4)?,
            status: row.get(5)?,
            path: row.get(6)?,
            created_at: row.get(7)?,
        })
    }
}

const SELECT_COLS: &str = "id, key, start_date, end_date, description, status, path, created_at";

/// List cycles, ordered by start_date desc.
pub fn list(conn: &Connection) -> Result<Vec<Cycle>> {
    let sql = format!("SELECT {SELECT_COLS} FROM cycles ORDER BY start_date DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], Cycle::from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read cycles")
}

/// Fetch a cycle by key. Returns `None` if no match.
pub fn get_by_key(conn: &Connection, key: &str) -> Result<Option<Cycle>> {
    let sql = format!("SELECT {SELECT_COLS} FROM cycles WHERE key = ?");
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_row([key], Cycle::from_row)
        .optional()
        .context("failed to query cycle")
}

pub struct AddArgs<'a> {
    pub key: &'a str,
    pub start_date: &'a str,
    pub end_date: &'a str,
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub path: Option<&'a str>,
}

/// Insert a new cycle. Fails if `key` already exists.
pub fn add(conn: &Connection, args: AddArgs) -> Result<Cycle> {
    let status = args.status.unwrap_or("planned");
    if !STATUSES.contains(&status) {
        bail!(
            "invalid status '{status}' (expected {})",
            STATUSES.join(", ")
        );
    }
    conn.execute(
        "INSERT INTO cycles (key, start_date, end_date, description, status, path) \
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            args.key,
            args.start_date,
            args.end_date,
            args.description,
            status,
            args.path,
        ],
    )
    .with_context(|| format!("failed to insert cycle {}", args.key))?;
    get_by_key(conn, args.key)?.with_context(|| format!("cycle {} missing after insert", args.key))
}

#[derive(Default)]
pub struct EditArgs<'a> {
    pub start_date: Option<&'a str>,
    pub end_date: Option<&'a str>,
    pub description: Option<&'a str>,
    pub status: Option<&'a str>,
    pub path: Option<&'a str>,
}

impl EditArgs<'_> {
    fn is_empty(&self) -> bool {
        self.start_date.is_none()
            && self.end_date.is_none()
            && self.description.is_none()
            && self.status.is_none()
            && self.path.is_none()
    }
}

pub fn edit(conn: &Connection, key: &str, args: EditArgs) -> Result<Cycle> {
    if args.is_empty() {
        bail!("no fields to update");
    }
    if get_by_key(conn, key)?.is_none() {
        bail!("cycle not found: {key}");
    }
    if let Some(s) = args.status {
        if !STATUSES.contains(&s) {
            bail!("invalid status '{s}' (expected {})", STATUSES.join(", "));
        }
    }
    conn.execute(
        "UPDATE cycles SET \
            start_date  = COALESCE(?, start_date), \
            end_date    = COALESCE(?, end_date), \
            description = COALESCE(?, description), \
            status      = COALESCE(?, status), \
            path        = COALESCE(?, path) \
         WHERE key = ?",
        params![
            args.start_date,
            args.end_date,
            args.description,
            args.status,
            args.path,
            key
        ],
    )
    .with_context(|| format!("failed to update cycle {key}"))?;
    get_by_key(conn, key)?.with_context(|| format!("cycle {key} missing after update"))
}

pub fn render_list(cycles: &[Cycle]) -> String {
    if cycles.is_empty() {
        return String::from("(no cycles)\n");
    }
    let key_w = cycles.iter().map(|c| c.key.len()).max().unwrap_or(4).max(4);
    let status_w = cycles
        .iter()
        .map(|c| c.status.len())
        .max()
        .unwrap_or(7)
        .max(7);

    let mut out = String::new();
    out.push_str(&format!(
        "{:<key_w$}  {:<status_w$}  start       end         description\n",
        "key", "status",
    ));
    for c in cycles {
        out.push_str(&format!(
            "{:<key_w$}  {:<status_w$}  {:<10}  {:<10}  {}\n",
            c.key,
            c.status,
            c.start_date,
            c.end_date,
            c.description.as_deref().unwrap_or(""),
        ));
    }
    out
}

pub fn render_show(c: &Cycle) -> String {
    let mut out = String::new();
    out.push_str(&format!("key:         {}\n", c.key));
    out.push_str(&format!("status:      {}\n", c.status));
    out.push_str(&format!("start_date:  {}\n", c.start_date));
    out.push_str(&format!("end_date:    {}\n", c.end_date));
    if let Some(p) = &c.path {
        out.push_str(&format!("path:        {p}\n"));
    }
    if let Some(d) = &c.description {
        out.push_str(&format!("description: {d}\n"));
    }
    out.push_str(&format!("created_at:  {}\n", c.created_at));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::workspace::root::ROOT_MARKER;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Connection) {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(ROOT_MARKER)).unwrap();
        let conn = db::open(tmp.path()).unwrap();
        (tmp, conn)
    }

    #[test]
    fn add_list_edit() {
        let (_tmp, conn) = setup();
        add(
            &conn,
            AddArgs {
                key: "C-15",
                start_date: "2026-04-13",
                end_date: "2026-04-18",
                description: Some("Ship + harden"),
                status: Some("active"),
                path: None,
            },
        )
        .unwrap();
        add(
            &conn,
            AddArgs {
                key: "C-16",
                start_date: "2026-04-20",
                end_date: "2026-04-25",
                description: None,
                status: None,
                path: None,
            },
        )
        .unwrap();

        let all = list(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].key, "C-16"); // start_date DESC

        edit(
            &conn,
            "C-16",
            EditArgs {
                status: Some("active"),
                description: Some("P1 features"),
                ..Default::default()
            },
        )
        .unwrap();
        let c = get_by_key(&conn, "C-16").unwrap().unwrap();
        assert_eq!(c.status, "active");
        assert_eq!(c.description.as_deref(), Some("P1 features"));
    }

    #[test]
    fn duplicate_key_errors() {
        let (_tmp, conn) = setup();
        add(
            &conn,
            AddArgs {
                key: "C-15",
                start_date: "2026-04-13",
                end_date: "2026-04-18",
                description: None,
                status: None,
                path: None,
            },
        )
        .unwrap();
        let err = add(
            &conn,
            AddArgs {
                key: "C-15",
                start_date: "2026-04-20",
                end_date: "2026-04-25",
                description: None,
                status: None,
                path: None,
            },
        );
        assert!(err.is_err());
    }
}
