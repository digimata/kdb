//! Tasks table access and display.
//!
//! A task has a per-project `seq` counter; its external id is
//! `{project_slug}-{seq}` (e.g. `hermaeus-42`). Statuses are
//! `open` | `in_progress` | `done` | `parked`.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

pub const STATUSES: &[&str] = &["open", "in_progress", "done", "parked"];

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub seq: i64,
    pub title: String,
    pub body: Option<String>,
    pub status: String,
    pub priority: i64,
    pub cycle_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

/// Task row joined with its project slug and optional cycle key.
#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    #[serde(flatten)]
    pub task: Task,
    pub project_slug: String,
    pub cycle_key: Option<String>,
}

impl TaskView {
    /// External task id: `{project_slug}-{seq}`.
    pub fn external_id(&self) -> String {
        format!("{}-{}", self.project_slug, self.task.seq)
    }
}

/// Parsed external task id.
#[derive(Debug, Clone)]
pub struct TaskId {
    pub project_slug: String,
    pub seq: i64,
}

impl TaskId {
    /// Parse `slug-seq` (splits on the last `-`).
    pub fn parse(s: &str) -> Result<Self> {
        let idx = s
            .rfind('-')
            .with_context(|| format!("invalid task id '{s}': expected slug-seq"))?;
        let (slug, seq_part) = s.split_at(idx);
        if slug.is_empty() {
            bail!("invalid task id '{s}': empty slug");
        }
        let seq: i64 = seq_part[1..]
            .parse()
            .with_context(|| format!("invalid task id '{s}': seq not an integer"))?;
        Ok(Self {
            project_slug: slug.to_string(),
            seq,
        })
    }
}

const SELECT_COLS: &str = "t.id, t.project_id, t.seq, t.title, t.body, \
    t.status, t.priority, t.cycle_id, t.parent_id, \
    t.created_at, t.updated_at, t.closed_at, p.slug, c.key";

fn view_from_row(row: &rusqlite::Row) -> rusqlite::Result<TaskView> {
    let task = Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        seq: row.get(2)?,
        title: row.get(3)?,
        body: row.get(4)?,
        status: row.get(5)?,
        priority: row.get(6)?,
        cycle_id: row.get(7)?,
        parent_id: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        closed_at: row.get(11)?,
    };
    Ok(TaskView {
        task,
        project_slug: row.get(12)?,
        cycle_key: row.get(13)?,
    })
}

/// Filters for [`list`].
#[derive(Debug, Default)]
pub struct ListFilters<'a> {
    pub statuses: Option<&'a [&'a str]>,
    pub project_slug: Option<&'a str>,
    pub cycle_key: Option<&'a str>,
    pub priority: Option<i64>,
    pub limit: Option<i64>,
}

/// List tasks matching the filters, ordered by priority then updated_at desc.
pub fn list(conn: &Connection, f: ListFilters) -> Result<Vec<TaskView>> {
    let mut sql = format!(
        "SELECT {SELECT_COLS} FROM tasks t \
         JOIN projects p ON p.id = t.project_id \
         LEFT JOIN cycles c ON c.id = t.cycle_id \
         WHERE 1=1"
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(statuses) = f.statuses {
        if statuses.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; statuses.len()].join(",");
        sql.push_str(&format!(" AND t.status IN ({placeholders})"));
        for s in statuses {
            args.push(Box::new(s.to_string()));
        }
    }
    if let Some(slug) = f.project_slug {
        sql.push_str(" AND p.slug = ?");
        args.push(Box::new(slug.to_string()));
    }
    if let Some(key) = f.cycle_key {
        sql.push_str(" AND c.key = ?");
        args.push(Box::new(key.to_string()));
    }
    if let Some(pri) = f.priority {
        sql.push_str(" AND t.priority = ?");
        args.push(Box::new(pri));
    }

    sql.push_str(" ORDER BY t.priority ASC, t.updated_at DESC");

    if let Some(limit) = f.limit {
        sql.push_str(" LIMIT ?");
        args.push(Box::new(limit));
    }

    let mut stmt = conn.prepare(&sql)?;
    let params_iter = rusqlite::params_from_iter(args.iter().map(|b| b.as_ref()));
    let rows = stmt.query_map(params_iter, view_from_row)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read tasks")
}

/// Resolve a parsed external id to a joined task row.
pub fn get(conn: &Connection, id: &TaskId) -> Result<Option<TaskView>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM tasks t \
         JOIN projects p ON p.id = t.project_id \
         LEFT JOIN cycles c ON c.id = t.cycle_id \
         WHERE p.slug = ? AND t.seq = ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_row(params![id.project_slug, id.seq], view_from_row)
        .optional()
        .context("failed to query task")
}

pub struct AddArgs<'a> {
    pub project_id: i64,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub priority: Option<i64>,
    pub cycle_id: Option<i64>,
    pub parent_id: Option<i64>,
}

/// Insert a new task, auto-assigning the per-project `seq` in a transaction.
pub fn add(conn: &mut Connection, args: AddArgs) -> Result<TaskView> {
    let priority = args.priority.unwrap_or(3);
    if !(1..=5).contains(&priority) {
        bail!("priority must be between 1 and 5");
    }

    let tx = conn.transaction()?;
    let seq: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE project_id = ?",
            [args.project_id],
            |row| row.get(0),
        )
        .context("failed to compute next seq")?;

    tx.execute(
        "INSERT INTO tasks \
         (project_id, seq, title, body, priority, cycle_id, parent_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        params![
            args.project_id,
            seq,
            args.title,
            args.body,
            priority,
            args.cycle_id,
            args.parent_id,
        ],
    )
    .context("failed to insert task")?;

    let slug: String = tx
        .query_row(
            "SELECT slug FROM projects WHERE id = ?",
            [args.project_id],
            |row| row.get(0),
        )
        .context("failed to read project slug")?;

    tx.commit()?;

    let id = TaskId {
        project_slug: slug,
        seq,
    };
    get(conn, &id)?.context("task missing after insert")
}

#[derive(Default)]
pub struct EditArgs<'a> {
    pub title: Option<&'a str>,
    pub body: Option<&'a str>,
    pub priority: Option<i64>,
    pub cycle_id: Option<Option<i64>>,
    pub parent_id: Option<Option<i64>>,
}

impl EditArgs<'_> {
    fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.body.is_none()
            && self.priority.is_none()
            && self.cycle_id.is_none()
            && self.parent_id.is_none()
    }
}

/// Update mutable fields on a task. `None` leaves the field unchanged;
/// for `cycle_id` / `parent_id`, `Some(None)` explicitly clears.
pub fn edit(
    conn: &Connection,
    id: &TaskId,
    args: EditArgs,
) -> Result<TaskView> {
    if args.is_empty() {
        bail!("no fields to update");
    }
    let existing = get(conn, id)?
        .with_context(|| format!("task not found: {}-{}", id.project_slug, id.seq))?;
    if let Some(pri) = args.priority {
        if !(1..=5).contains(&pri) {
            bail!("priority must be between 1 and 5");
        }
    }

    conn.execute(
        "UPDATE tasks SET \
            title      = COALESCE(?, title), \
            priority   = COALESCE(?, priority), \
            cycle_id   = CASE WHEN ? = 1 THEN ? ELSE cycle_id END, \
            parent_id  = CASE WHEN ? = 1 THEN ? ELSE parent_id END, \
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = ?",
        params![
            args.title,
            args.priority,
            args.cycle_id.is_some() as i64,
            args.cycle_id.and_then(|x| x),
            args.parent_id.is_some() as i64,
            args.parent_id.and_then(|x| x),
            existing.task.id,
        ],
    )
    .context("failed to update task")?;
    if let Some(body) = args.body {
        conn.execute(
            "UPDATE tasks SET body = ?, \
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE id = ?",
            params![body, existing.task.id],
        )?;
    }
    get(conn, id)?.context("task missing after update")
}

/// Set a task's status. `done` and `parked` stamp `closed_at`; other
/// statuses clear it.
pub fn set_status(
    conn: &Connection,
    id: &TaskId,
    status: &str,
) -> Result<TaskView> {
    if !STATUSES.contains(&status) {
        bail!(
            "invalid status '{status}' (expected {})",
            STATUSES.join(", ")
        );
    }
    let existing = get(conn, id)?
        .with_context(|| format!("task not found: {}-{}", id.project_slug, id.seq))?;

    let closed = matches!(status, "done" | "parked");
    conn.execute(
        "UPDATE tasks SET \
            status     = ?, \
            closed_at  = CASE WHEN ? = 1 \
                              THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') \
                              ELSE NULL END, \
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = ?",
        params![status, closed as i64, existing.task.id],
    )?;
    get(conn, id)?.context("task missing after status change")
}

fn status_glyph(status: &str) -> &'static str {
    match status {
        "open" => "[ ]",
        "in_progress" => "[~]",
        "done" => "[x]",
        "parked" => "[=]",
        _ => "[?]",
    }
}

/// Render a list of tasks as an aligned text table.
pub fn render_list(tasks: &[TaskView]) -> String {
    if tasks.is_empty() {
        return String::from("(no tasks)\n");
    }
    let id_strs: Vec<String> = tasks.iter().map(|t| t.external_id()).collect();
    let id_w = id_strs.iter().map(|s| s.len()).max().unwrap_or(4).max(4);

    let mut out = String::new();
    out.push_str(&format!(
        "{:<id_w$}  st    p  title\n",
        "id",
    ));
    for (t, id) in tasks.iter().zip(id_strs.iter()) {
        out.push_str(&format!(
            "{:<id_w$}  {}  {}  {}\n",
            id,
            status_glyph(&t.task.status),
            t.task.priority,
            t.task.title,
        ));
    }
    out
}

/// Render a single task as a human-readable block.
pub fn render_show(t: &TaskView) -> String {
    let mut out = String::new();
    out.push_str(&format!("id:         {}\n", t.external_id()));
    out.push_str(&format!("title:      {}\n", t.task.title));
    out.push_str(&format!("status:     {}\n", t.task.status));
    out.push_str(&format!("priority:   {}\n", t.task.priority));
    out.push_str(&format!("project:    {}\n", t.project_slug));
    if let Some(c) = &t.cycle_key {
        out.push_str(&format!("cycle:      {c}\n"));
    }
    out.push_str(&format!("created_at: {}\n", t.task.created_at));
    out.push_str(&format!("updated_at: {}\n", t.task.updated_at));
    if let Some(closed) = &t.task.closed_at {
        out.push_str(&format!("closed_at:  {closed}\n"));
    }
    if let Some(body) = &t.task.body {
        out.push_str("\n");
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::project::root::ROOT_MARKER;
    use crate::projects::{self, AddArgs as ProjAddArgs};
    use tempfile::TempDir;

    fn setup() -> (TempDir, Connection, i64) {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(ROOT_MARKER)).unwrap();
        let conn = db::open(tmp.path()).unwrap();
        let p = projects::add(
            &conn,
            ProjAddArgs {
                slug: "kdb",
                name: None,
                path: "projects/kdb",
                description: None,
            },
        )
        .unwrap();
        (tmp, conn, p.id)
    }

    #[test]
    fn parse_task_id() {
        let id = TaskId::parse("hermaeus-42").unwrap();
        assert_eq!(id.project_slug, "hermaeus");
        assert_eq!(id.seq, 42);

        let id = TaskId::parse("kdb-multi-word-1").unwrap();
        assert_eq!(id.project_slug, "kdb-multi-word");
        assert_eq!(id.seq, 1);

        assert!(TaskId::parse("nodash").is_err());
        assert!(TaskId::parse("slug-abc").is_err());
    }

    #[test]
    fn add_then_list_assigns_seq() {
        let (_tmp, mut conn, pid) = setup();
        let a = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "first",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
            },
        )
        .unwrap();
        assert_eq!(a.task.seq, 1);
        assert_eq!(a.external_id(), "kdb-1");

        let b = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "second",
                body: Some("body text"),
                priority: Some(2),
                cycle_id: None,
                parent_id: None,
            },
        )
        .unwrap();
        assert_eq!(b.task.seq, 2);
        assert_eq!(b.task.priority, 2);

        let all = list(&conn, ListFilters::default()).unwrap();
        assert_eq!(all.len(), 2);
        // priority 2 first
        assert_eq!(all[0].task.seq, 2);
    }

    #[test]
    fn set_status_transitions() {
        let (_tmp, mut conn, pid) = setup();
        let t = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "first",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
            },
        )
        .unwrap();
        let id = TaskId::parse(&t.external_id()).unwrap();

        let done = set_status(&conn, &id, "done").unwrap();
        assert_eq!(done.task.status, "done");
        assert!(done.task.closed_at.is_some());

        let reopened = set_status(&conn, &id, "open").unwrap();
        assert_eq!(reopened.task.status, "open");
        assert!(reopened.task.closed_at.is_none());

        assert!(set_status(&conn, &id, "bogus").is_err());
    }

    #[test]
    fn list_filters_by_status_and_project() {
        let (_tmp, mut conn, pid) = setup();
        let hermaeus = projects::add(
            &conn,
            ProjAddArgs {
                slug: "hermaeus",
                name: None,
                path: "projects/hermaeus",
                description: None,
            },
        )
        .unwrap();
        for title in ["a", "b"] {
            add(
                &mut conn,
                AddArgs {
                    project_id: pid,
                    title,
                    body: None,
                    priority: None,
                    cycle_id: None,
                    parent_id: None,
                },
            )
            .unwrap();
        }
        add(
            &mut conn,
            AddArgs {
                project_id: hermaeus.id,
                title: "c",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
            },
        )
        .unwrap();

        let kdb_only = list(
            &conn,
            ListFilters {
                project_slug: Some("kdb"),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(kdb_only.len(), 2);

        set_status(&conn, &TaskId::parse("kdb-1").unwrap(), "done").unwrap();
        let open_only = list(
            &conn,
            ListFilters {
                statuses: Some(&["open", "in_progress"]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(open_only.len(), 2);
    }

    #[test]
    fn edit_updates_fields() {
        let (_tmp, mut conn, pid) = setup();
        let t = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "orig",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
            },
        )
        .unwrap();
        let id = TaskId::parse(&t.external_id()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));

        let out = edit(
            &conn,
            &id,
            EditArgs {
                title: Some("new title"),
                priority: Some(1),
                body: Some("a body"),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.task.title, "new title");
        assert_eq!(out.task.priority, 1);
        assert_eq!(out.task.body.as_deref(), Some("a body"));
        assert_ne!(out.task.updated_at, t.task.updated_at);

        assert!(edit(&conn, &id, EditArgs::default()).is_err());
    }
}
