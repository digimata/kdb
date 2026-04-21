//! Tasks table access and display.
//!
//! A task has a per-project `seq` counter; its external id is
//! `{PROJECT_ALIAS}-{seq:04d}` (e.g. `HRM-0120`). Statuses are user-
//! customizable via the `task_statuses` table; the default seeded set
//! is `backlog` | `cycle` | `in_progress` | `parked` | `done`.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

// -------------------------------------------------------------
// projects/kdb/src/tasks.rs
//
// pub const DEFAULT_STATUSES                                L72
// pub fn is_closed_status()                                 L76
// pub const SEQ_WIDTH                                       L87
// pub const ORDER_KEY_WIDTH                                 L90
// pub fn format_external_id()                               L79
// pub fn default_order_key()                                L84
// const ALPHABET                                            L91
// fn rank()                                                 L93
// pub fn between()                                         L104
// fn key_before()                                          L116
// fn key_after()                                           L129
// fn key_between_two()                                     L144
// pub struct Task                                          L183
// pub struct TaskView                                      L201
//   pub fn external_id()                                   L211
// pub struct TaskId                                        L218
//   pub fn parse()                                         L225
// const SELECT_COLS                                        L243
// fn view_from_row()                                       L247
// pub struct ListFilters                                   L273
// pub fn list()                                            L282
// pub fn get()                                             L329
// pub struct ChildTask                                     L344
// pub fn children()                                        L353
// pub struct AddArgs                                       L379
// pub fn add()                                             L397
// pub struct EditArgs                                      L469
//   fn is_empty()                                          L478
// pub fn edit()                                            L489
// pub enum Side                                            L533
// pub enum MoveTarget                                      L540
// pub fn order_key_adjacent()                              L549
// pub fn move_task()                                       L577
// fn ensure_same_context()                                 L640
// enum Direction                                           L659
// fn neighbor_order()                                      L668
// pub fn set_status()                                      L709
// fn status_glyph()                                        L733
// pub fn render_list()                                     L744
// pub fn render_show()                                     L767
// mod tests                                                L810
// fn setup()                                               L817
// fn parse_task_id_uppercases_alias()                      L836
// fn external_id_is_zero_padded()                          L846
// fn add_auto_seq_then_explicit_seq()                      L853
// fn add_with_status()                                     L910
// fn set_status_transitions()                              L932
// fn children_are_ordered_by_order_key()                   L961
// fn render_show_includes_children_section()              L1029
// fn add_task()                                           L1069
// fn between_sits_strictly_between_various_pairs()        L1088
// fn move_task_before_and_after_reorders()                L1119
// fn move_task_top_and_bottom()                           L1167
// fn move_task_rejects_different_parent()                 L1204
// fn order_key_adjacent_after_sits_between_neighbors()    L1221
// -------------------------------------------------------------

/// Default seeded statuses. Users can add, rename, or remove these via
/// `kdb statuses` — treat this list as a fallback for display only.
pub const DEFAULT_STATUSES: &[&str] = &["backlog", "cycle", "in_progress", "parked", "done"];

/// Look up whether a status slug is marked `is_closed` in `task_statuses`.
/// Returns an error if the slug doesn't exist.
pub fn is_closed_status(conn: &Connection, slug: &str) -> Result<bool> {
    let flag: i64 = conn
        .query_row(
            "SELECT is_closed FROM task_statuses WHERE slug = ?",
            [slug],
            |row| row.get(0),
        )
        .with_context(|| format!("unknown task status '{slug}'"))?;
    Ok(flag != 0)
}

/// Zero-padding width for the `seq` portion of external ids.
pub const SEQ_WIDTH: usize = 4;

/// Zero-padding width for lexicographically sortable task order keys.
pub const ORDER_KEY_WIDTH: usize = 12;

/// Format an external task id: `{ALIAS}-{seq:04d}`.
pub fn format_external_id(alias: &str, seq: i64) -> String {
    format!("{alias}-{seq:0width$}", width = SEQ_WIDTH)
}

/// Default order key for newly created tasks (used when no explicit position is given).
pub fn default_order_key(seq: i64) -> String {
    format!("{seq:0width$}", width = ORDER_KEY_WIDTH)
}

/// Alphabet for lexicographic order keys. Byte-order sorted (`'0'..'9'` < `'a'..'z'`).
/// The migration backfill (`printf('%012d', seq)`) is a strict subset, so generated
/// keys interleave with legacy rows under plain string comparison.
const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

fn rank(b: u8) -> usize {
    match b {
        b'0'..=b'9' => (b - b'0') as usize,
        b'a'..=b'z' => 10 + (b - b'a') as usize,
        _ => unreachable!("order keys use alphabet 0-9a-z (got byte {b:#x})"),
    }
}

/// Generate an order key that sorts strictly between `prev` and `next` under byte
/// comparison. `None` means unbounded on that side. Panics if inputs are inconsistent
/// (`prev >= next`) or if the requested position is below an all-`0` key.
pub fn between(prev: Option<&str>, next: Option<&str>) -> String {
    match (prev, next) {
        (None, None) => String::from(ALPHABET[ALPHABET.len() / 2] as char),
        (None, Some(n)) => key_before(n),
        (Some(p), None) => key_after(p),
        (Some(p), Some(n)) => {
            debug_assert!(p < n, "between: prev must be < next ({p:?} vs {n:?})");
            key_between_two(p, n)
        }
    }
}

fn key_before(next: &str) -> String {
    let mut result = Vec::new();
    for &byte in next.as_bytes() {
        let r = rank(byte);
        if r > 0 {
            result.push(ALPHABET[r / 2]);
            return String::from_utf8(result).unwrap();
        }
        result.push(byte);
    }
    panic!("cannot produce key before all-min key: {next:?}");
}

fn key_after(prev: &str) -> String {
    let mut result = Vec::new();
    for &byte in prev.as_bytes() {
        let r = rank(byte);
        if r + 1 < ALPHABET.len() {
            let high = ALPHABET.len() - 1;
            result.push(ALPHABET[(r + high + 1) / 2]);
            return String::from_utf8(result).unwrap();
        }
        result.push(byte);
    }
    result.push(ALPHABET[ALPHABET.len() / 2]);
    String::from_utf8(result).unwrap()
}

fn key_between_two(prev: &str, next: &str) -> String {
    let a = prev.as_bytes();
    let b = next.as_bytes();
    let mut result: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        result.push(a[i]);
        i += 1;
    }
    let ar = a.get(i).copied().map(rank);
    let br = b.get(i).copied().map(rank);
    match (ar, br) {
        (Some(ar), Some(br)) => {
            debug_assert!(ar < br);
            if br - ar >= 2 {
                result.push(ALPHABET[(ar + br) / 2]);
                return String::from_utf8(result).unwrap();
            }
            result.push(ALPHABET[ar]);
            let rest = key_after(std::str::from_utf8(&a[i + 1..]).unwrap());
            result.extend_from_slice(rest.as_bytes());
            String::from_utf8(result).unwrap()
        }
        (None, Some(br)) => {
            if br > 0 {
                result.push(ALPHABET[br / 2]);
                return String::from_utf8(result).unwrap();
            }
            result.push(ALPHABET[0]);
            let rest = key_before(std::str::from_utf8(&b[i + 1..]).unwrap());
            result.extend_from_slice(rest.as_bytes());
            String::from_utf8(result).unwrap()
        }
        (Some(_), None) => unreachable!("key_between_two: next exhausted while prev < next"),
        (None, None) => unreachable!("key_between_two: prev == next"),
    }
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

    sql.push_str(" ORDER BY COALESCE(t.\"order\", printf('%012d', t.seq)) ASC, t.seq ASC");

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
    /// Explicit order key. When `None`, falls back to [`default_order_key`].
    pub order: Option<&'a str>,
}

/// Insert a new task, auto-assigning the per-project `seq` in a
/// transaction (unless [`AddArgs::seq`] is `Some`).
pub fn add(conn: &mut Connection, args: AddArgs) -> Result<TaskView> {
    let priority = args.priority.unwrap_or(3);
    if !(1..=5).contains(&priority) {
        bail!("priority must be between 1 and 5");
    }
    let status = args.status.unwrap_or("backlog");
    let closed = is_closed_status(conn, status)?;

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

    let order_key = args
        .order
        .map(str::to_string)
        .unwrap_or_else(|| default_order_key(seq));
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

/// Side of a sibling task when inserting/moving by position.
#[derive(Debug, Copy, Clone)]
pub enum Side {
    Before,
    After,
}

/// Relative position for [`move_task`].
#[derive(Debug, Clone)]
pub enum MoveTarget<'a> {
    Before(&'a TaskId),
    After(&'a TaskId),
    Top,
    Bottom,
}

/// Compute an order key for a new task inserted adjacent to `sibling` on the given side,
/// within `sibling`'s `(project_id, parent_id)` context.
pub fn order_key_adjacent(
    conn: &Connection,
    sibling: &TaskView,
    side: Side,
) -> Result<String> {
    let dir = match side {
        Side::Before => Direction::Prev,
        Side::After => Direction::Next,
    };
    let neighbor = neighbor_order(
        conn,
        sibling.task.project_id,
        sibling.task.parent_id,
        Some(&sibling.task.order),
        dir,
        // No task to exclude when inserting a new row.
        -1,
    )?;
    let pivot = sibling.task.order.as_str();
    let (prev, next) = match side {
        Side::Before => (neighbor.as_deref(), Some(pivot)),
        Side::After => (Some(pivot), neighbor.as_deref()),
    };
    Ok(between(prev, next))
}

/// Move a task to a new position within its `(project_id, parent_id)` context.
/// `Before`/`After` require the sibling to be in the same context; otherwise errors.
pub fn move_task(conn: &Connection, id: &TaskId, target: MoveTarget) -> Result<TaskView> {
    let task = get(conn, id)?
        .with_context(|| format!("task not found: {}", format_external_id(&id.alias, id.seq)))?;
    let project_id = task.task.project_id;
    let parent_id = task.task.parent_id;

    let new_order = match target {
        MoveTarget::Top => {
            let first = neighbor_order(conn, project_id, parent_id, None, Direction::First, task.task.id)?;
            between(None, first.as_deref())
        }
        MoveTarget::Bottom => {
            let last = neighbor_order(conn, project_id, parent_id, None, Direction::Last, task.task.id)?;
            between(last.as_deref(), None)
        }
        MoveTarget::Before(sib_id) => {
            let sib = get(conn, sib_id)?.with_context(|| {
                format!("sibling not found: {}", format_external_id(&sib_id.alias, sib_id.seq))
            })?;
            ensure_same_context(&task, &sib)?;
            if sib.task.id == task.task.id {
                return Ok(task);
            }
            let prev = neighbor_order(
                conn,
                project_id,
                parent_id,
                Some(&sib.task.order),
                Direction::Prev,
                task.task.id,
            )?;
            between(prev.as_deref(), Some(&sib.task.order))
        }
        MoveTarget::After(sib_id) => {
            let sib = get(conn, sib_id)?.with_context(|| {
                format!("sibling not found: {}", format_external_id(&sib_id.alias, sib_id.seq))
            })?;
            ensure_same_context(&task, &sib)?;
            if sib.task.id == task.task.id {
                return Ok(task);
            }
            let next = neighbor_order(
                conn,
                project_id,
                parent_id,
                Some(&sib.task.order),
                Direction::Next,
                task.task.id,
            )?;
            between(Some(&sib.task.order), next.as_deref())
        }
    };

    conn.execute(
        "UPDATE tasks SET \"order\" = ?, \
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = ?",
        params![new_order, task.task.id],
    )
    .context("failed to update task order")?;
    get(conn, id)?.context("task missing after move")
}

fn ensure_same_context(task: &TaskView, sib: &TaskView) -> Result<()> {
    if task.task.project_id != sib.task.project_id {
        bail!(
            "cannot move {}: target {} is in a different project",
            task.external_id(),
            sib.external_id()
        );
    }
    if task.task.parent_id != sib.task.parent_id {
        bail!(
            "cannot move {}: target {} has a different parent",
            task.external_id(),
            sib.external_id()
        );
    }
    Ok(())
}

#[derive(Copy, Clone)]
enum Direction {
    First,
    Last,
    Prev,
    Next,
}

/// Look up a neighboring task's order key within a `(project_id, parent_id)` context.
/// `exclude_id` is always filtered out so moving a task past its own current slot works.
fn neighbor_order(
    conn: &Connection,
    project_id: i64,
    parent_id: Option<i64>,
    pivot: Option<&str>,
    dir: Direction,
    exclude_id: i64,
) -> Result<Option<String>> {
    let parent_pred = match parent_id {
        Some(_) => "parent_id = ?",
        None => "parent_id IS NULL",
    };
    let (cmp, order) = match dir {
        Direction::First => ("", "ASC"),
        Direction::Last => ("", "DESC"),
        Direction::Prev => ("AND \"order\" < ?", "DESC"),
        Direction::Next => ("AND \"order\" > ?", "ASC"),
    };
    let sql = format!(
        "SELECT \"order\" FROM tasks \
         WHERE project_id = ? AND {parent_pred} AND id != ? {cmp} \
         ORDER BY \"order\" {order} LIMIT 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    params_vec.push(Box::new(project_id));
    if let Some(pid) = parent_id {
        params_vec.push(Box::new(pid));
    }
    params_vec.push(Box::new(exclude_id));
    if let Some(p) = pivot {
        params_vec.push(Box::new(p.to_string()));
    }
    let params_iter = rusqlite::params_from_iter(params_vec.iter().map(|b| b.as_ref()));
    stmt.query_row(params_iter, |row| row.get::<_, String>(0))
        .optional()
        .context("failed to read neighbor order")
}

/// Set a task's status. `done` and `parked` stamp `closed_at`; other
/// statuses clear it.
pub fn set_status(conn: &Connection, id: &TaskId, status: &str) -> Result<TaskView> {
    let closed = is_closed_status(conn, status)?;
    let existing = get(conn, id)?
        .with_context(|| format!("task not found: {}", format_external_id(&id.alias, id.seq)))?;

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
        "backlog" => "[ ]",
        "cycle" => "[>]",
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
                order: None,
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
                order: None,
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
                order: None,
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
                order: None,
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
                order: None,
            },
        )
        .unwrap();
        let id = TaskId::parse(&t.external_id()).unwrap();

        let done = set_status(&conn, &id, "done").unwrap();
        assert_eq!(done.task.status, "done");
        assert!(done.task.closed_at.is_some());

        let reopened = set_status(&conn, &id, "backlog").unwrap();
        assert_eq!(reopened.task.status, "backlog");
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
                order: None,
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
                order: None,
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
                order: None,
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
                order: None,
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
                order: None,
            },
        )
        .unwrap();

        let kids = children(&conn, parent.task.id).unwrap();
        let rendered = render_show(&parent, &[], &kids);
        assert!(rendered.contains("children:"));
        assert!(rendered.contains(&kids[0].id));
        assert!(rendered.contains("ord="));
    }

    fn add_task(conn: &mut Connection, pid: i64, title: &str, parent: Option<i64>) -> TaskView {
        add(
            conn,
            AddArgs {
                project_id: pid,
                title,
                body: None,
                priority: None,
                cycle_id: None,
                parent_id: parent,
                seq: None,
                status: None,
                order: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn between_sits_strictly_between_various_pairs() {
        let cases: &[(Option<&str>, Option<&str>)] = &[
            (None, None),
            (Some("000000000001"), Some("000000000002")),
            (Some("a"), Some("b")),
            (Some("a"), Some("c")),
            (Some("a"), Some("az")),
            (None, Some("m")),
            (Some("m"), None),
            (Some("abc"), Some("abd")),
            (Some("0"), Some("z")),
            (Some("zzz"), None),
        ];
        for &(prev, next) in cases {
            let mid = between(prev, next);
            if let Some(p) = prev {
                assert!(
                    p < mid.as_str(),
                    "between({prev:?}, {next:?}) = {mid:?} is not > {p:?}",
                );
            }
            if let Some(n) = next {
                assert!(
                    mid.as_str() < n,
                    "between({prev:?}, {next:?}) = {mid:?} is not < {n:?}",
                );
            }
        }
    }

    #[test]
    fn move_task_before_and_after_reorders() {
        let (_tmp, mut conn, pid) = setup();
        let a = add_task(&mut conn, pid, "a", None);
        let b = add_task(&mut conn, pid, "b", None);
        let c = add_task(&mut conn, pid, "c", None);

        let a_id = TaskId::parse(&a.external_id()).unwrap();
        let b_id = TaskId::parse(&b.external_id()).unwrap();
        let c_id = TaskId::parse(&c.external_id()).unwrap();

        // Move c before a → order: c, a, b
        let _ = move_task(&conn, &c_id, MoveTarget::Before(&a_id)).unwrap();
        let listed = list(
            &conn,
            ListFilters {
                statuses: None,
                project_slug: None,
                cycle_key: None,
                priority: None,
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(
            listed.iter().map(|t| t.task.title.as_str()).collect::<Vec<_>>(),
            vec!["c", "a", "b"],
        );

        // Move a after b → order: c, b, a
        let _ = move_task(&conn, &a_id, MoveTarget::After(&b_id)).unwrap();
        let listed = list(
            &conn,
            ListFilters {
                statuses: None,
                project_slug: None,
                cycle_key: None,
                priority: None,
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(
            listed.iter().map(|t| t.task.title.as_str()).collect::<Vec<_>>(),
            vec!["c", "b", "a"],
        );
    }

    #[test]
    fn move_task_top_and_bottom() {
        let (_tmp, mut conn, pid) = setup();
        let a = add_task(&mut conn, pid, "a", None);
        let _b = add_task(&mut conn, pid, "b", None);
        let _c = add_task(&mut conn, pid, "c", None);
        let a_id = TaskId::parse(&a.external_id()).unwrap();

        let _ = move_task(&conn, &a_id, MoveTarget::Bottom).unwrap();
        let listed = list(
            &conn,
            ListFilters {
                statuses: None,
                project_slug: None,
                cycle_key: None,
                priority: None,
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(listed.last().unwrap().task.title, "a");

        let _ = move_task(&conn, &a_id, MoveTarget::Top).unwrap();
        let listed = list(
            &conn,
            ListFilters {
                statuses: None,
                project_slug: None,
                cycle_key: None,
                priority: None,
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(listed.first().unwrap().task.title, "a");
    }

    #[test]
    fn move_task_rejects_different_parent() {
        let (_tmp, mut conn, pid) = setup();
        let parent = add_task(&mut conn, pid, "parent", None);
        let child = add_task(&mut conn, pid, "child", Some(parent.task.id));
        let root = add_task(&mut conn, pid, "root", None);

        let child_id = TaskId::parse(&child.external_id()).unwrap();
        let root_id = TaskId::parse(&root.external_id()).unwrap();

        let err = move_task(&conn, &child_id, MoveTarget::Before(&root_id)).unwrap_err();
        assert!(
            err.to_string().contains("different parent"),
            "unexpected error: {err}",
        );
    }

    #[test]
    fn order_key_adjacent_after_sits_between_neighbors() {
        let (_tmp, mut conn, pid) = setup();
        let a = add_task(&mut conn, pid, "a", None);
        let b = add_task(&mut conn, pid, "b", None);

        let key_after_a = order_key_adjacent(&conn, &a, Side::After).unwrap();
        assert!(key_after_a > a.task.order, "{key_after_a} > {}", a.task.order);
        assert!(key_after_a < b.task.order, "{key_after_a} < {}", b.task.order);

        let key_before_a = order_key_adjacent(&conn, &a, Side::Before).unwrap();
        assert!(key_before_a < a.task.order, "{key_before_a} < {}", a.task.order);
    }
}
