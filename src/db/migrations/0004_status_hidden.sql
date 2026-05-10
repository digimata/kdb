-- 0004: hidden flag on statuses.
--
-- A `hidden` status renders in the materialized index as a heading +
-- count + summary command line (no table, no per-task files). It's
-- distinct from `is_closed`: parked is closed but still shown; done
-- is both closed AND hidden by default so it doesn't blow up the
-- index as completed work accumulates.

ALTER TABLE task_statuses
  ADD COLUMN is_hidden INTEGER NOT NULL DEFAULT 0
  CHECK (is_hidden IN (0, 1));

ALTER TABLE project_statuses
  ADD COLUMN is_hidden INTEGER NOT NULL DEFAULT 0
  CHECK (is_hidden IN (0, 1));

UPDATE task_statuses SET is_hidden = 1 WHERE slug = 'done';
