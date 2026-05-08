---
status: in_progress
started: 2026-05-08
---

# kdb cleanup + delete semantics rework

Rolls up four asks the user stacked after the initial subtask/dotted-ID
landing in commits `a6008b7` (feat) and `e2dc517` (release):

1. **Decluttering migrations.** Fold the recently-added 0004 migration
   schema back into 0003 and delete 0004. User runs the migration
   manually on their local DB.
2. **No frontmatter on rendered `.tasks/index.md`.** Strip the YAML
   frontmatter block from the materialized index. Per-task
   `T-NNNN.md` frontmatter stays (not in scope; user asked specifically
   about the index).
3. **Real delete, not parking.** Today `kdb tasks delete` calls
   `set_status(parked)`. User wants soft-delete via a dedicated
   `deleted_at` column (orthogonal to status), with `restore` to undo
   and `--hard` to permanently remove. `rm` should be an alias.
4. **Bulk purge for done tasks.** Done tasks accumulate forever in
   `index.md`'s "Done (N)" count. Need a way to actually clear them.
   Likely a `tasks purge --status done [-P slug]` (and
   `tasks purge --deleted` for soft-deleted rows), with a `--dry-run`.

## Current state on disk (uncommitted)

```
M src/db/migrations/0003_customizable_statuses.sql   # tasks_new now has child_seq + CHECK + UNIQUE; backfill copies seq->seq, child_seq->NULL for top-level. Still missing deleted_at.
D src/db/migrations/0004_task_child_seq.sql          # deleted
M src/db/mod.rs                                      # MIGRATIONS array trimmed back to 3 entries
M src/materialize.rs                                 # frontmatter block removed from render_index; chrono::Utc import removed
```

`src/tasks.rs` was reverted back to its committed state (deleted_at
field was added then removed mid-edit). All other Rust code is
unchanged from `e2dc517`.

These uncommitted edits build & test (the migration fold + frontmatter
strip), but I haven't run `cargo test` since the deleted_at revert.
**Verify with `cargo test` before extending.**

## Decisions already locked in

- IDs and rendering follow the model from the prior commits — top-level
  `KDB-0030`, children `KDB-0030.1`, grandchildren `KDB-0030.1.2`,
  arbitrary depth.
- `tasks delete` → soft-delete by setting `deleted_at = now()`.
- Soft-delete and restore both **cascade** to the entire descendant
  subtree.
- `tasks delete <id> --hard` → permanent DELETE of the row + subtree
  (the self-FK `parent_id REFERENCES tasks(id)` has no ON DELETE CASCADE,
  so we walk the subtree manually inside a transaction with
  `defer_foreign_keys = ON`, or recursive-CTE delete bottom-up).
- `task_labels` already has `ON DELETE CASCADE` (see
  `0001_relational.sql:60-61`) — labels clean themselves up.
- `deleted_at` is **orthogonal to `status`**. A done task can still be
  soft-deleted; a backlog task can be soft-deleted; etc. Default lists
  filter `deleted_at IS NULL`.
- `rm` is an alias for `delete`. `d` alias stays. Both go through the
  same code path.
- Migration policy: fold the `deleted_at` column into 0003 (don't add a
  new migration file). User runs the schema delta manually. **Don't
  bump `MIGRATIONS.len()`** — leaving it at 3 is fine; the user's
  existing DB has `user_version=4` from the now-deleted migration and
  must be reset to `3` manually.

## What remains

### Step A — finish the cleanup commit (small)

1. Rebuild `cargo test` from current uncommitted state to confirm green.
2. Reinstall: `cargo install --path .`.
3. Tell the user the manual SQL for their local DB:
   ```sql
   PRAGMA user_version = 3;
   ```
   (Their schema is already correct from 0.32.0 having run 0004; only
   the version pragma needs re-aligning to match what fresh installs
   compute.)
4. Commit as something like `chore(db): fold child_seq into 0003;
   strip frontmatter from rendered index`.

### Step B — delete semantics rework (the bulk of the work)

#### B1. Schema

Add `deleted_at TEXT` column to `tasks_new` in
`0003_customizable_statuses.sql` (still no new migration file). Update
the INSERT INTO tasks_new to carry `deleted_at` (or NULL on backfill
since the old shape didn't have it).

User's manual SQL on top of step A:
```sql
ALTER TABLE tasks ADD COLUMN deleted_at TEXT;
```

#### B2. Rust model (`src/tasks.rs`)

- `Task` gains `deleted_at: Option<String>`.
- `SELECT_COLS` adds `t.deleted_at` (mind the column index — `view_from_row` reads positional indices; the `dotted` column is currently at index 17 because it's last; insert `deleted_at` *before* the join columns or *after* dotted — pick one and update indices consistently).
- `view_from_row` reads the new column.
- `ListFilters` gains `pub include_deleted: bool` (default false).
- In `list`, append `AND t.deleted_at IS NULL` unless
  `include_deleted` is set.
- `get`/`get_by_row_id` still return the row regardless of deletion
  state — callers can inspect `deleted_at`. (We need to read deleted
  rows during restore.)

New functions:

```rust
pub fn soft_delete(conn: &mut Connection, id: &TaskId) -> Result<TaskView>;
pub fn restore(conn: &mut Connection, id: &TaskId) -> Result<TaskView>;
pub fn hard_delete(conn: &mut Connection, id: &TaskId) -> Result<TaskView>;

pub struct PurgeFilters<'a> {
    pub project_slug: Option<&'a str>,
    pub status: Option<&'a str>,
    pub deleted_only: bool,   // include rows where deleted_at IS NOT NULL
    pub dry_run: bool,
}
pub fn purge(conn: &mut Connection, f: PurgeFilters) -> Result<Vec<TaskView>>;
```

Implementation notes:

- All four cascade across the subtree. Use the recursive CTE pattern
  from `descendants()` (`tasks.rs:480` ish) — collect ids, then
  UPDATE/DELETE in a transaction.
- `hard_delete` and `purge` need `defer_foreign_keys = ON` *inside* the
  transaction, OR delete bottom-up (children before parents). The
  recursive-CTE-in-DELETE form works in SQLite ≥ 3.8.3:
  ```sql
  WITH RECURSIVE sub(id) AS (
    SELECT ?1 UNION ALL
    SELECT t.id FROM tasks t JOIN sub s ON t.parent_id = s.id
  )
  DELETE FROM tasks WHERE id IN (SELECT id FROM sub);
  ```
  Verify FKs don't fire on intermediate states — easiest is to wrap in
  `BEGIN; PRAGMA defer_foreign_keys = ON; ...; COMMIT;`.

#### B3. CLI (`src/cmd.rs` + `src/main.rs`)

Replace `tasks_delete`:

```rust
pub fn tasks_delete(id: String, hard: bool) -> Result<()> { ... }
pub fn tasks_restore(id: String) -> Result<()> { ... }
pub fn tasks_purge(
    project: Option<String>,
    status: Option<String>,
    deleted: bool,
    dry_run: bool,
) -> Result<()> { ... }
```

`TasksCmd` enum (`src/main.rs:239`) updates:

- `Delete { id, hard }` — add `#[arg(long)] hard: bool`. Add `rm` alias to the variant: `#[command(alias = "rm", alias = "d")]`.
- New `Restore { id }`.
- New `Purge { project, status, deleted, dry_run }` with mutually-exclusive selectors validated in `tasks_purge`.

#### B4. Materialize behavior

- `materialize` already lists via `tasks::list`. With `include_deleted`
  defaulting to false, soft-deleted tasks naturally drop out of
  `index.md`. Counts (in_progress, cycle, backlog, parked, done) reflect
  non-deleted tasks. ✓
- "Done (N)" count: same — only non-deleted done tasks count.

#### B5. Tests

Update:

- `tests/cli.rs` `tasks_delete_and_d_alias_soft_delete_to_parked` — rename
  and rewrite. Soft-delete should NOT change status; should set
  `deleted_at`; should hide from default `tasks list` and `index.md`;
  should be visible with `tasks list --include-deleted` (need to add this flag too — see B2 — and a corresponding CLI flag in `TasksCmd::List`).
- New tests:
  - `tasks_delete_soft_then_restore`.
  - `tasks_delete_hard_removes_row_and_subtree`.
  - `tasks_purge_status_done_clears_done_tasks`.
  - `tasks_purge_dry_run_lists_without_deleting`.
  - Lib-level: `soft_delete_cascades_to_subtree`,
    `hard_delete_removes_descendants`,
    `purge_respects_project_filter`.

### Step C — manual SQL for the user (final)

After both commits land, the user runs locally on
`~/Documents/digimata/.kdb/index.db`:

```sql
ALTER TABLE tasks ADD COLUMN deleted_at TEXT;
PRAGMA user_version = 3;
```

(If they prefer, do it with `sqlite3 ~/Documents/digimata/.kdb/index.db <<EOF`).

Verify with `kdb render --all` after.

### Step D — version bump + CHANGELOG

Bump `Cargo.toml` to 0.33.0; CHANGELOG entry covering both the
schema-fold/no-frontmatter cleanup and the delete-semantics rework.
Probably best as one CHANGELOG section since they ship together. Two
commits is fine: one for code, one for release.

## Open questions for the user (revisit after compaction)

1. **Should `tasks list --include-deleted` show deleted rows mixed
   with live ones, or only deleted?** I'd default to *mixed* (deleted
   rows get a marker in the title or a dim render), so the flag is a
   simple toggle. Confirm.
2. **Should `tasks restore` reset the `order` key or keep the
   pre-deletion value?** Default: keep — order is preserved across
   delete/restore so users get back what they had.
3. **`tasks purge` with no filter at all** — what should happen?
   Refuse and require at least one selector? Or purge all soft-deleted
   rows by default (equivalent to `--deleted`)? I'd refuse by default
   to prevent accidents.
4. **CHANGELOG entry: bundle into 0.32.0 as a follow-up section, or
   release as 0.33.0?** Probably 0.33.0 since 0.32.0 is already
   committed and the user installs from local; clean version bump
   keeps `kdb --version` honest.

## Files to revisit

- `src/db/migrations/0003_customizable_statuses.sql` — schema source of truth.
- `src/db/mod.rs` — MIGRATIONS array (already at 3).
- `src/tasks.rs` — Task struct, SELECT_COLS, view_from_row, list,
  ListFilters, new soft/hard/restore/purge functions, tests at the
  bottom.
- `src/cmd.rs` — tasks_delete, new tasks_restore, new tasks_purge.
- `src/main.rs` — TasksCmd enum: Delete, Restore, Purge variants.
- `src/materialize.rs` — already updated (frontmatter strip). Verify
  `tasks::list` filtering does what we expect for default render.
- `tests/cli.rs` — rewrite `tasks_delete_and_d_alias_soft_delete_to_parked`,
  add new tests.
- `CHANGELOG.md` — 0.33.0 section.
- `Cargo.toml` — version bump.

## Resume checklist

When you pick this up:

1. `cd /Users/andjones/Documents/digimata/projects/kdb`
2. `git status` — should show the four uncommitted files listed above.
3. `cargo test` — confirm green from the cleanup edits.
4. Execute Step A (commit + install + tell user the `PRAGMA` SQL).
5. Implement Step B in the order B1 → B2 → B3 → B5 → B4-validate.
6. Step C — give the user the ALTER TABLE SQL.
7. Step D — version bump + CHANGELOG + second commit.
