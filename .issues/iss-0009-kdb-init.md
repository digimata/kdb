---
id: 9
title: kdb init and .kdb/ Directory
status: done
priority: critical
labels:
  - feat
closed_on: 2026-02-17
---

# 0009 :: kdb init and .kdb/ Directory

## Intent

Switch from a single `kdb.toml` marker file to a `.kdb/` directory structure (like `.git/`), and add a `kdb init` command to scaffold it.

## Current Behavior

- Root discovery walks upward looking for a `kdb.toml` file.
- Config is a single flat file at the project root.
- No init command exists — users manually create `kdb.toml`.

## Proposed Behavior

### `kdb init`

Running `kdb init` in a directory creates:

```
.kdb/
  config.toml     # project config (replaces kdb.toml)
```

Default `config.toml`:

```toml
[project]
name = "<directory name>"
```

Future additions to `.kdb/` (not part of this issue):

```
.kdb/
  index/          # cached parse/link index
  modules/        # installed kdb packages (see #0008)
  lock.toml       # dependency lockfile
```

### Root Discovery

- Change `ROOT_MARKER` from `"kdb.toml"` to `".kdb"`.
- Check for directory existence instead of file existence.
- Add `config_path()` helper that returns `.kdb/config.toml`.

### Changes Required

1. **`src/root.rs`** — Change marker to `.kdb/`, use `is_dir()` instead of `is_file()`, add `config_path()`.
2. **`src/main.rs`** — Add `Init` variant to `Command` enum.
3. **`src/cmd.rs`** — Add `init()` function that creates `.kdb/config.toml`.
4. **`src/lib.rs`** — Update doc comment.
5. **`tests/root.rs`** — Update fixtures to create `.kdb/config.toml` instead of `kdb.toml`.
6. **`tests/cli.rs`, `tests/index.rs`, `tests/lsp.rs`** — Same fixture updates.
7. **Migrate this repo** — Replace `kdb.toml` with `.kdb/config.toml`.

### Error Cases

- `kdb init` in a directory that already has `.kdb/` should error with a clear message.
- `kdb init` should fail if it can't create the directory (permissions, etc.).

## Resolution

- Root marker switched from `kdb.toml` to `.kdb/` directory.
- Added `config_path()` helper in `src/root.rs` for `.kdb/config.toml`.
- Added `kdb init` command in CLI (`src/main.rs`, `src/cmd.rs`).
- Migrated repository root marker to `.kdb/config.toml`.
- Updated root-dependent fixtures across `tests/root.rs`, `tests/cli.rs`, `tests/index.rs`, and `tests/lsp.rs`.
- Added init command tests for success and existing `.kdb/` error path.
