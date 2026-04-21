//! Tasks table access and display.
//!
//! A task has a per-project `seq` counter; its external id is
//! `{PROJECT_ALIAS}-{seq:04d}` (e.g. `HRM-0120`). Statuses are
//! `open` | `in_progress` | `done` | `parked`.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

// -------------------------------------------
// projects/kdb/src/tasks.rs
//
// pub const STATUSES                      L45
// pub const SEQ_WIDTH                     L48
// pub fn format_external_id()             L51
// pub struct Task                         L56
// pub struct TaskView                     L73
//   pub fn external_id()                  L83
// pub struct TaskId                       L90
//   pub fn parse()                        L97
// const SELECT_COLS                      L115
// fn view_from_row()                     L119
// pub struct ListFilters                 L144
// pub fn list()                          L153
// pub fn get()                           L200
// pub struct AddArgs                     L213
// pub fn add()                           L229
// pub struct EditArgs                    L296
//   fn is_empty()                        L305
// pub fn edit()                          L316
// pub fn set_status()                    L360
// fn status_glyph()                      L384
// pub fn render_list()                   L395
// pub fn render_show()                   L418
// mod tests                              L447
// fn setup()                             L454
// fn parse_task_id_uppercases_alias()    L473
// fn external_id_is_zero_padded()        L483
// fn add_auto_seq_then_explicit_seq()    L490
// fn add_with_status()                   L544
// fn set_status_transitions()            L565
// -------------------------------------------

pub const STATUSES: &[&str] = &["open", "in_progress", "done", "parked"];

/// Zero-padding width for the `seq` portion of external ids.
pub const SEQ_WIDTH: usize = 4;

/// Zero-padding width for lexicographically sortable task order keys.
pub const ORDER_KEY_WIDTH: usize = 12;

/// Format an external task id: `{ALIAS}-{seq:04d}`.
pub fn format_external_id(alias: &str, seq: i64) -> String {
    format!("{alias}-{seq:0width$}", width = SEQ_WIDTH)
}

/// Default order key for newly created tasks.
pub fn default_order_key(seq: i64) -> String {
    format!("{seq:0width$}", width = ORDER_KEY_WIDTH)
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
    pub seq: i64,
    pub title: String,
    pub body: Option<String>,
    pub status: String,
    pub priority: i64,
    pub order: String,
    pub cycle_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

/// Task row joined with its project slug/alias and optional cycle key.
#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    #[serde(flatten)]
    pub task: Task,
    pub project_slug: String,
    pub project_alias: String,
    pub cycle_key: Option<String>,
}

impl TaskView {
    /// External task id: `{PROJECT_ALIAS}-{seq:04d}`.
    pub fn external_id(&self) -> String {
        format_external_id(&self.project_alias, self.task.seq)
    }
}

/// Parsed external task id. `alias` is always uppercased.
#[derive(Debug, Clone)]
pub struct TaskId {
    pub alias: String,
    pub seq: i64,
}

impl TaskId {
    /// Parse `ALIAS-seq` (splits on the last `-`). Alias is uppercased.
    pub fn parse(s: &str) -> Result<Self> {
        let idx = s
            .rfind('-')
            .with_context(|| format!("invalid task id '{s}': expected ALIAS-seq"))?;
        let (prefix, seq_part) = s.split_at(idx);
        if prefix.is_empty() {
            bail!("invalid task id '{s}': empty alias");
        }
        let seq: i64 = seq_part[1..]
            .parse()
            .with_context(|| format!("invalid task id '{s}': seq not an integer"))?;
        Ok(Self {
            alias: prefix.to_ascii_uppercase(),
            seq,
        })
    }
}

const SELECT_COLS: &str = "t.id, t.project_id, t.seq, t.title, t.body, \
    t.status, t.priority, COALESCE(t.\"order\", printf('%012d', t.seq)), t.cycle_id, t.parent_id, \
    t.created_at, t.updated_at, t.closed_at, p.slug, p.alias, c.key";

fn view_from_row(row: &rusqlite::Row) -> rusqlite::Result<TaskView> {
    let task = Task {
        id: row.get(0)?,
        project_id: row.get(1)?,
        seq: row.get(2)?,
        title: row.get(3)?,
        body: row.get(4)?,
        status: row.get(5)?,
        priority: row.get(6)?,
        order: row.get(7)?,
        cycle_id: row.get(8)?,
        parent_id: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        closed_at: row.get(12)?,
    };
    Ok(TaskView {
        task,
        project_slug: row.get(13)?,
        project_alias: row.get(14)?,
        cycle_key: row.get(15)?,
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

/// Resolve a parsed external id (alias + seq) to a joined task row.
pub fn get(conn: &Connection, id: &TaskId) -> Result<Option<TaskView>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM tasks t \
         JOIN projects p ON p.id = t.project_id \
         LEFT JOIN cycles c ON c.id = t.cycle_id \
         WHERE p.alias = ? AND t.seq = ?"
    );
    let mut stmt = conn.prepare(&sql)?;
    stmt.query_row(params![id.alias, id.seq], view_from_row)
        .optional()
        .context("failed to query task")
}

/// Compact child-task payload for task view output.
#[derive(Debug, Clone, Serialize)]
pub struct ChildTask {
    pub id: String,
    pub status: String,
    pub priority: i64,
    pub title: String,
    pub order: String,
}

/// List direct child tasks for a parent, ordered by lexical `order` key.
pub fn children(conn: &Connection, parent_task_id: i64) -> Result<Vec<ChildTask>> {
    let mut stmt = conn.prepare(
        "SELECT p.alias, t.seq, t.status, t.priority, t.title, \
                COALESCE(t.\"order\", printf('%012d', t.seq)) AS task_order \
         FROM tasks t \
         JOIN projects p ON p.id = t.project_id \
         WHERE t.parent_id = ? \
         ORDER BY task_order ASC, t.seq ASC",
    )?;

    let rows = stmt.query_map(params![parent_task_id], |row| {
        let alias: String = row.get(0)?;
        let seq: i64 = row.get(1)?;
        Ok(ChildTask {
            id: format_external_id(&alias, seq),
            status: row.get(2)?,
            priority: row.get(3)?,
            title: row.get(4)?,
            order: row.get(5)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to query child tasks")
}

pub struct AddArgs<'a> {
    pub project_id: i64,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub priority: Option<i64>,
    pub cycle_id: Option<i64>,
    pub parent_id: Option<i64>,
    /// Explicit seq for migrations/imports. When `None`, auto-assigned
    /// as `max(seq) + 1` within the project.
    pub seq: Option<i64>,
    /// Explicit status (defaults to `open`).
    pub status: Option<&'a str>,
}

/// Insert a new task, auto-assigning the per-project `seq` in a
/// transaction (unless [`AddArgs::seq`] is `Some`).
pub fn add(conn: &mut Connection, args: AddArgs) -> Result<TaskView> {
    let priority = args.priority.unwrap_or(3);
    if !(1..=5).contains(&priority) {
        bail!("priority must be between 1 and 5");
    }
    let status = args.status.unwrap_or("open");
    if !STATUSES.contains(&status) {
        bail!(
            "invalid status '{status}' (expected {})",
            STATUSES.join(", ")
        );
    }

    let tx = conn.transaction()?;
    let seq: i64 = match args.seq {
        Some(s) => {
            if s < 1 {
                bail!("seq must be >= 1");
            }
            s
        }
        None => tx
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE project_id = ?",
                [args.project_id],
                |row| row.get(0),
            )
            .context("failed to compute next seq")?,
    };

    let closed = matches!(status, "done" | "parked");
    let order_key = default_order_key(seq);
    tx.execute(
        "INSERT INTO tasks \
         (project_id, seq, title, body, status, priority, \"order\", cycle_id, parent_id, closed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, \
                 CASE WHEN ? = 1 \
                       THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') \
                       ELSE NULL END)",
        params![
            args.project_id,
            seq,
            args.title,
            args.body,
            status,
            priority,
            order_key,
            args.cycle_id,
            args.parent_id,
            closed as i64,
        ],
    )
    .with_context(|| format!("failed to insert task (seq={seq})"))?;

    let alias: String = tx
        .query_row(
            "SELECT alias FROM projects WHERE id = ?",
            [args.project_id],
            |row| row.get(0),
        )
        .context("failed to read project alias")?;

    tx.commit()?;

    let id = TaskId { alias, seq };
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
pub fn edit(conn: &Connection, id: &TaskId, args: EditArgs) -> Result<TaskView> {
    if args.is_empty() {
        bail!("no fields to update");
    }
    let existing = get(conn, id)?
        .with_context(|| format!("task not found: {}", format_external_id(&id.alias, id.seq)))?;
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
pub fn set_status(conn: &Connection, id: &TaskId, status: &str) -> Result<TaskView> {
    if !STATUSES.contains(&status) {
        bail!(
            "invalid status '{status}' (expected {})",
            STATUSES.join(", ")
        );
    }
    let existing = get(conn, id)?
        .with_context(|| format!("task not found: {}", format_external_id(&id.alias, id.seq)))?;

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
    out.push_str(&format!("{:<id_w$}  st    p  title\n", "id",));
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

/// Render a single task as a human-readable block. `label_slugs` are
/// joined with `, ` and shown in the metadata section when non-empty.
pub fn render_show(t: &TaskView, label_slugs: &[&str], children: &[ChildTask]) -> String {
    let mut out = String::new();
    out.push_str(&format!("id:         {}\n", t.external_id()));
    out.push_str(&format!("title:      {}\n", t.task.title));
    out.push_str(&format!("status:     {}\n", t.task.status));
    out.push_str(&format!("priority:   {}\n", t.task.priority));
    out.push_str(&format!("order:      {}\n", t.task.order));
    out.push_str(&format!("project:    {}\n", t.project_slug));
    if let Some(c) = &t.cycle_key {
        out.push_str(&format!("cycle:      {c}\n"));
    }
    if !label_slugs.is_empty() {
        out.push_str(&format!("labels:     {}\n", label_slugs.join(", ")));
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
    if !children.is_empty() {
        out.push_str("\nchildren:\n");
        for child in children {
            out.push_str(&format!(
                "- {}  {}  p{}  ord={}  {}\n",
                child.id,
                status_glyph(&child.status),
                child.priority,
                child.order,
                child.title,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::projects::{self, AddArgs as ProjAddArgs};
    use crate::workspace::root::ROOT_MARKER;
    use tempfile::TempDir;

    fn setup() -> (TempDir, Connection, i64) {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(ROOT_MARKER)).unwrap();
        let conn = db::open(tmp.path()).unwrap();
        let p = projects::add(
            &conn,
            ProjAddArgs {
                slug: "kdb",
                alias: "KDB",
                name: None,
                path: "projects/kdb",
                description: None,
            },
        )
        .unwrap();
        (tmp, conn, p.id)
    }

    #[test]
    fn parse_task_id_uppercases_alias() {
        let id = TaskId::parse("hrm-0120").unwrap();
        assert_eq!(id.alias, "HRM");
        assert_eq!(id.seq, 120);

        assert!(TaskId::parse("nodash").is_err());
        assert!(TaskId::parse("slug-abc").is_err());
    }

    #[test]
    fn external_id_is_zero_padded() {
        assert_eq!(format_external_id("HRM", 1), "HRM-0001");
        assert_eq!(format_external_id("KDB", 120), "KDB-0120");
        assert_eq!(format_external_id("SWF", 99999), "SWF-99999");
    }

    #[test]
    fn add_auto_seq_then_explicit_seq() {
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
                seq: None,
                status: None,
            },
        )
        .unwrap();
        assert_eq!(a.task.seq, 1);
        assert_eq!(a.external_id(), "KDB-0001");

        let b = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "explicit",
                body: Some("body text"),
                priority: Some(2),
                cycle_id: None,
                parent_id: None,
                seq: Some(120),
                status: None,
            },
        )
        .unwrap();
        assert_eq!(b.task.seq, 120);
        assert_eq!(b.external_id(), "KDB-0120");

        let c = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "after explicit",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
                seq: None,
                status: None,
            },
        )
        .unwrap();
        assert_eq!(c.task.seq, 121);
    }

    #[test]
    fn add_with_status() {
        let (_tmp, mut conn, pid) = setup();
        let t = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "done one",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
                seq: None,
                status: Some("done"),
            },
        )
        .unwrap();
        assert_eq!(t.task.status, "done");
        assert!(t.task.closed_at.is_some());
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
                seq: None,
                status: None,
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
    }

    #[test]
    fn children_are_ordered_by_order_key() {
        let (_tmp, mut conn, pid) = setup();
        let parent = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "parent",
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: None,
                seq: None,
                status: None,
            },
        )
        .unwrap();
        let child_a = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "child a",
                body: None,
                priority: Some(2),
                cycle_id: None,
                parent_id: Some(parent.task.id),
                seq: None,
                status: None,
            },
        )
        .unwrap();
        let child_b = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "child b",
                body: None,
                priority: Some(1),
                cycle_id: None,
                parent_id: Some(parent.task.id),
                seq: None,
                status: None,
            },
        )
        .unwrap();

        conn.execute(
            "UPDATE tasks SET \"order\" = ? WHERE id = ?",
            params!["b", child_a.task.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE tasks SET \"order\" = ? WHERE id = ?",
            params!["a", child_b.task.id],
        )
        .unwrap();

        let kids = children(&conn, parent.task.id).unwrap();
        assert_eq!(kids.len(), 2);
        assert_eq!(kids[0].id, child_b.external_id());
        assert_eq!(kids[0].order, "a");
        assert_eq!(kids[1].id, child_a.external_id());
        assert_eq!(kids[1].order, "b");
    }

    #[test]
    fn render_show_includes_children_section() {
        let (_tmp, mut conn, pid) = setup();
        let parent = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "parent",
                body: Some("details"),
                priority: Some(2),
                cycle_id: None,
                parent_id: None,
                seq: None,
                status: None,
            },
        )
        .unwrap();
        let _child = add(
            &mut conn,
            AddArgs {
                project_id: pid,
                title: "child",
                body: None,
                priority: Some(1),
                cycle_id: None,
                parent_id: Some(parent.task.id),
                seq: None,
                status: None,
            },
        )
        .unwrap();

        let kids = children(&conn, parent.task.id).unwrap();
        let rendered = render_show(&parent, &[], &kids);
        assert!(rendered.contains("children:"));
        assert!(rendered.contains(&kids[0].id));
        assert!(rendered.contains("ord="));
    }
}
