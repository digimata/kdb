---
id: 20
title: kdb deps Command
status: proposed
priority: high
labels:
  - feat
---

# ISS-0020 :: kdb deps Command

## Intent

List outbound dependencies of a file. Answers "what does this file reach for?"

## Usage

```
kdb deps <file>
kdb deps <file> --json
```

## Output

```
$ kdb deps docs/tutorial.md
docs/hooks.md
docs/components.md#Props
docs/state.md

$ kdb deps docs/tutorial.md --json
[
  {"file": "docs/hooks.md", "anchor": null},
  {"file": "docs/components.md", "anchor": "Props"},
  {"file": "docs/state.md", "anchor": null}
]
```

## Flags

- `--json` — structured output for programmatic consumption

## Implementation

- Build `VaultIndex`, look up the file, iterate its `links` list
- Resolve each link target to a file path (reuse existing resolution logic)
- Deduplicate and sort output

## Scope

Markdown-only for now. Code import listing (phase 3) will extend this with `use`/`import` parsing and file resolution per language.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Deps` subcommand |
| `src/cmd.rs` | Add `cmd::deps()` entrypoint |
