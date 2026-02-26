---
id: 55
title: "`refs -s` grouped output and `--files` flag"
status: done
priority: high
labels:
  - enhancement
  - refs
---

# ISS-0055 :: `refs -s` grouped output and `--files` flag

## Problem

When agents use `kdb refs <file> -s <symbol>`, the flat output lists every reference line with the file path repeated. To get unique files, agents consistently pipe through `awk -F: '{print $1}' | sort -u`, wasting 1-2 extra tool calls per query.

Observed in ISS-0047 benchmarks — every kdb agent run on T1 (find callers) did this.

## Changes

### 1. Grouped output (new default)

Group references by file using the same `── file` heading format as `kdb symbols` multi-file output.

```
$ kdb refs src/token.rs -s Token

── src/token.rs
  28:1   pub struct Token(pub usize);

── src/event/source.rs
  5:18   use crate::{Interest, Registry, Token};
  42:9   token: Token,
  58:9   token: Token,

── src/io_source.rs
  1:30   use crate::{event, Interest, Registry, Token};
  32:9   token: Token,
```

Agents get the file list (headings) and line detail in one call.

### 2. `--files` / `-l` flag

Unique file paths only, one per line. Matches rg's `-l`/`--files-with-matches` convention.

```
$ kdb refs src/token.rs -s Token --files
src/token.rs
src/event/source.rs
src/io_source.rs
```

### 3. No change to `--json`

JSON output already structured — no change needed.
