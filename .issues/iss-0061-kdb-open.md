---
id: 61
title: "kdb open — resolve kdb:// URIs and open in editor"
status: proposed
priority: low
labels:
  - feature
  - cli
---

## Problem

No way to open a `kdb://` URI from the command line. Currently requires manually stripping the scheme and calling the editor.

## Proposal

Add `kdb open <uri>` command:

```
kdb open marina/dossiers/214-marc-andreessen.md
kdb open kdb://marina/dossiers/214-marc-andreessen.md
```

- Strips `kdb://` scheme if present
- Resolves path relative to project root
- Opens in configured editor

### Editor config

Add `[editor]` section to `.kdb/config.toml`:

```toml
[editor]
command = "zed -a {path}"
```

`{path}` is interpolated with the resolved absolute path. Fallback chain: config → `$EDITOR {path}` → `open {path}`.
