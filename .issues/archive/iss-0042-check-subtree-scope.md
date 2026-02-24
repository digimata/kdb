---
id: 42
title: "kdb check: subtree scoping"
status: done
priority: medium
labels:
  - feat
---

# ISS-0042 :: kdb check: subtree scoping

## Intent

`kdb check <path>` should scope its output to only report broken links and orphans within the given subtree, not the entire vault.

## Current behavior

The `path` argument is only used to discover the vault root via `find_root()`. Once found, the full vault index is built and all files are checked. Running `kdb check crates/agent/` in a large repo reports broken links from `docs/` and everywhere else.

## Expected behavior

```bash
# Only report issues in crates/agent/ and its descendants
kdb check crates/agent/

# Still check the full vault when no path is given
kdb check
```

## Notes

- The index still needs to be built vault-wide (links can cross subtree boundaries)
- The scoping should filter the *report output*, not the index build
- A link from `crates/agent/foo.md` → `docs/bar.md` should still be checked, but a broken link in `docs/baz.md` should not appear
