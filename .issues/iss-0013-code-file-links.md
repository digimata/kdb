---
id: 13
title: Code File Links
status: proposed
priority: high
labels:
  - feat
---
> -----------------------------------
> .issues/iss-0013-code-file-links.md
>
> ISS-0013 :: Code File Links    L20
>   • Intent                     L22
>   • Examples                   L26
>   • Behavior                   L34
>   • Open Questions             L42
> -----------------------------------


# ISS-0013 :: Code File Links

## Intent

Links should be able to target non-markdown files (source code, config, etc.) with line number anchors, using the same syntax GitHub uses.

## Examples

```markdown
[root discovery](src/root.rs#L17)
[find_root function](src/root.rs#L17-L25)
[[src/root.rs#L17]]
```

## Behavior

- Go-to-definition should open the file at the specified line.
- Line range anchors (`#L10-L20`) should select/highlight the range.
- Autocomplete should suggest non-markdown files in the project.
- `kdb check` should validate that the file exists and the line numbers are in range.
- Hover could preview the referenced lines.

## Open Questions

- Should we also support symbol-based anchors for code files (e.g. `src/root.rs#find_root`)? Would need language-aware parsing.
- What file types to support? All files, or a configurable allowlist?
- How does this interact with `.gitignore` / `kdb ignore` patterns?
