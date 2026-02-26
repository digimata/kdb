---
id: 56
title: "`tree` should print absolute root path"
status: done
priority: medium
labels:
  - feat
---

# ISS-0056 :: `tree` should print absolute root path

## Problem

AI agents using kdb typically start with `kdb tree` to explore a directory. The output shows relative paths:

```
src/sys
├── unix
│   ├── mod.rs
```

When the agent then needs to `Read` a file (which requires an absolute path), it has to guess the absolute prefix and frequently gets it wrong — e.g. `/Users/foo/repo/src/sys/mod.rs` instead of `/Users/foo/Documents/repos/repo/src/sys/mod.rs`.

Baseline agents avoid this because `Glob` returns absolute paths, anchoring all subsequent Read calls. The kdb agent skips Glob (uses kdb commands instead), so it never gets the real absolute path.

Observed in ISS-0047 benchmarks — kdb agents on T2 (explain architecture) wasted 3-4 calls per repo on wrong-path retries.

## Proposal

Print the absolute path as the tree root:

```
/Users/foo/Documents/repos/mio/src/sys
├── unix
│   ├── mod.rs
│   ├── selector
...
```

This gives the agent a single anchor to derive absolute paths for all files in the tree.
