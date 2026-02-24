---
id: 33
title: Remove outline command (redundant with symbols)
status: proposed
priority: medium
labels:
  - cleanup
  - cli
---

# ISS-0033 :: Remove outline command

## Intent

`kdb outline <file>` prints the heading tree for a markdown file. `kdb symbols <file>` on a markdown file outputs the same headings. They're the same data with slightly different rendering. Remove `outline` to reduce command surface.

## What changes

- Remove `Outline` subcommand from `src/main.rs`
- Remove `cmd::outline()` from `src/cmd.rs`
- If the indented tree rendering is preferred for some use cases, add a `--tree` flag to `kdb symbols` for markdown files

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Remove `Outline` variant |
| `src/cmd.rs` | Remove `outline()` function |
| `tests/cli.rs` | Remove/update outline tests |
| `README.md` | Remove outline from commands list |
