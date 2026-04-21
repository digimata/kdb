-- Add lexical ordering key for manual task ordering.

ALTER TABLE tasks ADD COLUMN "order" TEXT;

UPDATE tasks
SET "order" = printf('%012d', seq)
WHERE "order" IS NULL;

CREATE INDEX idx_tasks_project_parent_order
  ON tasks(project_id, parent_id, "order");
