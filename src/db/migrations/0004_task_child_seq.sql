-- Subtasks no longer consume the per-project `seq` counter. Top-level
-- tasks keep `seq`; children get `child_seq` (1-based, unique within
-- a `parent_id`). External ids are dotted: `KDB-0030.1.2` walks the
-- parent chain. See .issues for context.

PRAGMA foreign_keys = OFF;

CREATE TABLE tasks_new (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  project_id  INTEGER NOT NULL REFERENCES projects(id),
  seq         INTEGER,
  child_seq   INTEGER,
  title       TEXT NOT NULL,
  body        TEXT,
  status      TEXT NOT NULL DEFAULT 'backlog'
              REFERENCES task_statuses(slug) ON UPDATE CASCADE,
  priority    INTEGER NOT NULL DEFAULT 3 CHECK (priority BETWEEN 1 AND 5),
  "order"     TEXT,
  cycle_id    INTEGER REFERENCES cycles(id),
  parent_id   INTEGER REFERENCES tasks(id),
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  closed_at   TEXT,
  CHECK ((parent_id IS NULL AND seq IS NOT NULL AND child_seq IS NULL)
      OR (parent_id IS NOT NULL AND seq IS NULL AND child_seq IS NOT NULL)),
  UNIQUE (project_id, seq),
  UNIQUE (parent_id, child_seq)
);

-- Top-level rows: copy seq verbatim, child_seq stays NULL.
INSERT INTO tasks_new (id, project_id, seq, child_seq, title, body, status, priority,
                       "order", cycle_id, parent_id, created_at, updated_at, closed_at)
  SELECT id, project_id, seq, NULL, title, body, status, priority,
         "order", cycle_id, parent_id, created_at, updated_at, closed_at
    FROM tasks
   WHERE parent_id IS NULL;

-- Children: drop seq, allocate child_seq via row_number among siblings.
INSERT INTO tasks_new (id, project_id, seq, child_seq, title, body, status, priority,
                       "order", cycle_id, parent_id, created_at, updated_at, closed_at)
  SELECT id, project_id, NULL,
         ROW_NUMBER() OVER (PARTITION BY parent_id
                            ORDER BY COALESCE("order", printf('%012d', seq)), seq),
         title, body, status, priority, "order",
         cycle_id, parent_id, created_at, updated_at, closed_at
    FROM tasks
   WHERE parent_id IS NOT NULL;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX idx_tasks_status_pri
  ON tasks(project_id, status, priority, updated_at);
CREATE INDEX idx_tasks_project_parent_order
  ON tasks(project_id, parent_id, "order");

PRAGMA foreign_keys = ON;
