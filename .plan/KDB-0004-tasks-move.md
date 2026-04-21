---
title: "KDB-0004 — Explicit task ordering + relative move commands"
date: 2026-04-20
status: draft
affects: "kdb tasks — ordering, move, add --before/--after"
---

## Context

The `"order"` column already exists on `tasks` (migration `0002_tasks_order.sql`), backfilled as `printf('%012d', seq)` with an index on `(project_id, parent_id, "order")`. `tasks::default_order_key()` assigns `{seq:012d}` to every new row, `tasks::children()` reads siblings ordered by the key, and `render_show` surfaces `order:` plus a `children:` block.

What's missing is the user-facing surface:

1. No way to change a task's position except by editing SQL directly.
2. `tasks::list()` still sorts by `priority ASC, updated_at DESC` — project views don't render in `order`.
3. `tasks add` always appends (key = padded seq); you can't insert at a specific slot.
4. No generator for a key between two existing keys.

This plan adds (a) a `between()` helper for lex-sortable key generation, (b) a `kdb tasks move` subcommand, (c) `--before`/`--after` on `kdb tasks add`, and (d) order-based sort in `list()` when scoped to a single project.

## Design

### Ordering scope (list context)

An "ordering context" is the set of siblings a key must sort uniquely within. For a task with `project_id=P, parent_id=X`:

- `X IS NULL` → root-level list for project P
- `X IS NOT NULL` → children of task X

Move operations always resolve within the target task's current context. `--before/--after OTHER` requires OTHER to be in the same context as the task being moved; otherwise error. (Moving between contexts = `tasks edit --parent`, out of scope here.)

### Key alphabet + `between()`

Alphabet: `0123456789abcdefghijklmnopqrstuvwxyz` (36 chars). Byte order matches lex order, and the existing `printf('%012d', seq)` backfill uses a strict subset (`0-9`), so generated keys interleave cleanly with legacy rows.

Algorithm for `between(prev: Option<&str>, next: Option<&str>) -> String`:

- `prev = None` acts as lower sentinel (`""`); `next = None` acts as upper sentinel (conceptually `z+`).
- Walk both strings char-by-char, keeping the common prefix.
- At the first divergent position:
  - If `next_char - prev_char > 1`: emit prefix + midpoint char → done.
  - If `next_char - prev_char == 1`: emit prefix + prev_char, then recurse on `(prev_tail, None)` (extend prev).
  - If `prev` has ended and `next` hasn't: find midpoint in `(0, next_char)` and append.
  - If `next` has ended and `prev` hasn't: find midpoint in `(prev_char, z)` and append.
- Invariant: result satisfies `prev < result < next` under byte comparison.

Boundary: between(None, None) → `"h"` (a middle char), so the first manually-positioned task in an empty context gets a short key.

**Rebalancing**: not scoped. Keys grow O(log₂ insertions_between_same_pair) — fine for ≪1M inserts between fixed neighbors. If it ever matters, add `kdb tasks rebalance` later.

### Move resolution

`move_task(conn, task_id, target)` where `target` is one of:

- `Before(other_id)` → new_order = `between(prev_of(other), other.order)`
- `After(other_id)`  → new_order = `between(other.order, next_of(other))`
- `Top`              → new_order = `between(None, first_in_context.order)`
- `Bottom`           → new_order = `between(last_in_context.order, None)`

`prev_of`/`next_of` are queried within the task's `(project_id, parent_id)` context, excluding the task being moved (so `move --after self-neighbor` is idempotent).

### `list()` sort

Change the default `ORDER BY` to `t."order" ASC, t.seq ASC`. Priority becomes a filter and display column, not a sort key. This matches the task's "task lists render in `order`" acceptance criterion. Cross-project listings share the same sort — since `order` is only unique within a context, ties fall back to `seq`, which is stable.

`render_list` stays as-is; `p` column still shows priority.

### `tasks add --before/--after`

When either flag is set:

- Resolve the target task, assert same project (and inherit `parent_id` from target when `--parent` not given).
- Compute new order key via `between()` against the target's neighbor.
- Pass the computed key through `AddArgs::order` (new field) instead of falling back to `default_order_key(seq)`.

`--before` and `--after` are mutually exclusive; neither → current append behaviour.

## Changes

1. **`src/tasks.rs`**
   - Add `mod order;` or inline: `pub fn between(prev: Option<&str>, next: Option<&str>) -> String` + private helpers (`midpoint_char`, constants for `MIN_CHAR='0'`, `MAX_CHAR='z'`).
   - Add `MoveTarget` enum: `Before(TaskId) | After(TaskId) | Top | Bottom`.
   - Add `pub fn move_task(conn: &Connection, id: &TaskId, target: MoveTarget) -> Result<TaskView>`. Resolves the task, reads its context `(project_id, parent_id)`, looks up neighbor order keys, calls `between()`, updates the row, bumps `updated_at`.
   - Extend `AddArgs` with `pub order: Option<String>`; `add()` uses it when Some, else falls back to `default_order_key(seq)`.
   - Change `list()` `ORDER BY` clause to `t."order" ASC, t.seq ASC`.
   - Unit tests: `between_is_between` (property-style: random prev/next pairs → result sorts strictly between), `move_before_updates_order`, `move_top_puts_first`, `move_bottom_puts_last`, `move_rejects_cross_context`, `add_before_inserts_key_between`.

2. **`src/cmd.rs`**
   - Add `pub fn tasks_move(id: String, before: Option<String>, after: Option<String>, top: bool, bottom: bool) -> Result<()>`. Validate exactly one target specified, call `tasks::move_task`, re-materialize the project index.
   - Update `tasks_add` signature to take `before: Option<String>`, `after: Option<String>`. Resolve target, compute order key, thread through `AddArgs`.

3. **`src/main.rs`**
   - New `TasksCmd::Move { id, before, after, top, bottom }` variant with mutually-exclusive `ArgGroup` for the four target options.
   - Extend `TasksCmd::Add` with `--before`/`--after` (mutually exclusive group). Wire both to the updated `cmd::tasks_add`.

4. **`tests/cli.rs`** — end-to-end integration tests:
   - `tasks_move_before_reorders_list` — add three tasks, move the third before the first, assert `tasks list` output order.
   - `tasks_move_top_and_bottom` — smoke test for `--top` / `--bottom`.
   - `tasks_add_after_inserts_between` — add two, add a third with `--after <first>`, list order matches.
   - `tasks_move_rejects_different_parent` — cross-context move errors cleanly.

5. **`CHANGELOG.md`** — new `## [0.30.0]` section: "Add `kdb tasks move` (`--before/--after/--top/--bottom`) and `kdb tasks add --before/--after` for manual task ordering. Project task lists now render in `order`."

6. **`Cargo.toml`** — bump version `0.29.0` → `0.30.0` (minor, new feature per SOP-C02).

7. **Global CLAUDE.md** (`~/Documents/digimata/.claude/CLAUDE.md`, kdb section) — add the new commands to the `kdb tasks` workflow block. (Deferred until after implementation to keep diffs reviewable.)

## Files touched

```
┌───────────────────────────────────┬──────────────────────────────────────────┐
│               File                │                  Action                  │
├───────────────────────────────────┼──────────────────────────────────────────┤
│ src/tasks.rs                      │ Edit — between(), move_task(), AddArgs   │
│                                   │        .order, list() sort, tests        │
│ src/cmd.rs                        │ Edit — tasks_move(), tasks_add signature │
│ src/main.rs                       │ Edit — TasksCmd::Move, Add flags         │
│ tests/cli.rs                      │ Edit — 4 new integration tests           │
│ CHANGELOG.md                      │ Edit — 0.30.0 entry                      │
│ Cargo.toml                        │ Edit — version bump                      │
│ ~/Documents/digimata/.claude/     │ Edit — add move/reorder to kdb tasks CLI │
│   CLAUDE.md                       │        reference (after implementation)  │
└───────────────────────────────────┴──────────────────────────────────────────┘
```

No new dependencies. No new migrations (0002 is sufficient).

## Verification

1. `cargo test` — all existing tests pass, new unit + integration tests pass.
2. `cargo clippy --all-targets` — clean.
3. Manual smoke against real DB:
   ```
   kdb tasks list -P kdb
   kdb tasks move KDB-4 --before KDB-1
   kdb tasks list -P kdb          # KDB-4 first
   kdb tasks move KDB-4 --bottom
   kdb tasks list -P kdb          # KDB-4 last
   kdb tasks add "scratch" -P kdb --after KDB-2
   kdb tasks view <new-id>         # order key sits between KDB-2 and its successor
   kdb tasks move KDB-4 --after KDB-99   # error: not in same context
   ```
4. `kdb check` — materialized `.tasks/index.md` renders in the new order, no broken links.
5. Output of `kdb tasks view KDB-4 --json` includes `"order"` with the new key.
