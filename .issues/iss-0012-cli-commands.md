---
id: 12
title: CLI Commands
status: in_progress
priority: high
labels:
  - feat
---

# 0012 :: CLI Commands

## Intent

Full CLI surface area for kdb.

## Commands

```
kdb check              # compile — report all errors/warnings
kdb outline <file>     # print heading tree
kdb refs <file>#<head> # find all references to a heading
kdb orphans            # list orphan files
kdb stubs              # list empty stubs
kdb graph              # output dependency graph (dot format)
kdb graph --cluster    # detect clusters of related knowledge
kdb init               # initialize a kdb project (creates .kdb/config.toml)
kdb fmt                # normalize link formats, fix slugs
```

## Status

- [x] `kdb check`
- [x] `kdb outline`
- [x] `kdb lsp`
- [x] `kdb init`
- [ ] `kdb refs`
- [ ] `kdb orphans`
- [ ] `kdb stubs`
- [ ] `kdb graph`
- [ ] `kdb graph --cluster`
- [ ] `kdb fmt`
