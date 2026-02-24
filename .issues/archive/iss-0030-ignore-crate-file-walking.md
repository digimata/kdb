---
id: 30
title: Use ripgrep's `ignore` crate for file walking
status: proposed
priority: high
labels:
  - perf
  - infra
---

# ISS-0030 :: Use `ignore` crate for file walking

## Intent

Replace our hand-rolled file discovery with ripgrep's `ignore` crate. It handles `.gitignore`, nested ignore files, symlink following, and parallel directory walking out of the box. Faster and more correct than maintaining our own walker.

## Implementation Plan

## Context

- File discovery is currently duplicated in `src/index/mod.rs` and `src/fmt/mod.rs` using `walkdir::WalkDir`, local `IGNORED_DIRS`, custom `GlobSet` compilation, and manual `path_is_ignored` checks.
- This approach misses important ignore semantics that `ignore` already handles correctly (nested `.gitignore`, `.ignore`, negation patterns, and parent traversal).
- We already have config-based ignores via `[index].ignore` in `.kdb/config.toml` (`src/config.rs`), so this migration must preserve that behavior as the project-local ignore mechanism.
- Existing ignore behavior is covered in `tests/index.rs`, `tests/fmt.rs`, and `tests/cli.rs`; these provide the baseline pattern to extend rather than replacing test style.
- `iss-0031` still handles parse fan-out; this issue can adopt `WalkParallel` now for traversal while keeping parse semantics unchanged.

## Changes

1. Add discovery backend dependency
   - What file: `Cargo.toml`
   - What it does: add `ignore` as a direct dependency for repo walking.
   - Key decisions or trade-offs: keep `walkdir` for now because `src/resolve/mod.rs` still uses it; we can remove `walkdir` in a follow-up when all walkers are migrated.

2. Add a shared `ignore`-based discovery helper
   - What file: `src/discovery.rs` (new), `src/lib.rs`
   - What it does: centralize walker construction (`WalkBuilder`) and normalized relative-path collection so `index` and `fmt` use one code path.
   - Key decisions or trade-offs: use `WalkParallel` from the same helper and collect matches into a shared buffer, then sort before returning so CLI/test output remains deterministic.

3. Migrate markdown indexing discovery
   - What file: `src/index/mod.rs`
   - What it does: replace `discover_markdown_files`/manual ignore filtering with the shared discovery helper + markdown-extension predicate, backed by `WalkParallel`.
   - Key decisions or trade-offs: preserve current indexing semantics (skip unreadable/non-UTF8 files, only `.md`) while changing traversal to parallel discovery.

4. Migrate formatter discovery
   - What file: `src/fmt/mod.rs`
   - What it does: replace `discover_code_files_in_scope` walking/filtering logic with the shared discovery helper for workspace and scoped path formatting, backed by `WalkParallel`.
   - Key decisions or trade-offs: keep language filtering in `fmt` (via `language_for_path`) so formatter ownership stays local and behavior parity is clear.

5. Wire config ignores into walker setup
   - What file: `src/config.rs` (and discovery plumbing call sites)
   - What it does: keep `[index].ignore` from `.kdb/config.toml` applied alongside native `ignore` crate behavior (`.gitignore`, `.ignore`).
   - Key decisions or trade-offs: keep one project-local ignore source (config) to avoid split ownership between config and another ignore file.

6. Extend discovery-focused tests
   - What file: `tests/index.rs`, `tests/fmt.rs`, `tests/cli.rs`
   - What it does: add coverage for `.gitignore` behavior (including nested ignore files + negation) and confirm existing config ignores still apply.
   - Key decisions or trade-offs: prefer small tempdir fixtures that isolate one ignore rule each; this keeps failures diagnosable and avoids brittle integration-only assertions.

## Files touched

```
┌──────────────────────┬──────────────────────────────────────────────────────────┐
│         File         │                          Action                          │
├──────────────────────┼──────────────────────────────────────────────────────────┤
│ Cargo.toml           │ Edit(add `ignore` dependency)                            │
│ src/lib.rs           │ Edit(register discovery module)                           │
│ src/discovery.rs     │ Create(shared WalkBuilder + rel-path collection helpers) │
│ src/index/mod.rs     │ Edit(use shared ignore-based markdown discovery)          │
│ src/fmt/mod.rs       │ Edit(use shared ignore-based code discovery)              │
│ src/config.rs        │ Edit(keep config as project-local ignore source)          │
│ tests/index.rs       │ Edit(add `.gitignore` index coverage)                     │
│ tests/fmt.rs         │ Edit(add formatter discovery ignore coverage)             │
│ tests/cli.rs         │ Edit(add end-to-end ignore behavior checks)               │
└──────────────────────┴──────────────────────────────────────────────────────────┘
```

## Verification

- Run `cargo test --test index` to verify markdown discovery behavior and ignore semantics.
- Run `cargo test --test fmt` to verify formatter discovery behavior under ignore rules.
- Run `cargo test --test cli` for end-to-end `kdb check`/`kdb fmt` behavior with config and ignore files.
- Run full suite with `cargo test` to catch regressions in modules still using old walkers.
- Repeat `cargo test --test index` and `cargo test --test fmt` at least twice locally to smoke-check deterministic ordering under parallel walk collection.
- Manual sanity checks in a temp project:
  1. add a root and nested `.gitignore` and confirm ignored files are not indexed/formatted.
  2. add `[index].ignore` patterns in `.kdb/config.toml` and confirm they apply alongside `.gitignore`.
  3. add a negated `.gitignore` pattern and confirm un-ignored files are discovered.
