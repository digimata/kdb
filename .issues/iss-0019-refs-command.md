---
id: 19
title: kdb refs Command
status: proposed
priority: high
labels:
  - feat
---

# 0019 :: kdb refs Command

## Intent

Find all inbound references to a file or heading. Answers "who links to this?"

## Usage

```
kdb refs <file>
kdb refs <file>#<heading>
kdb refs <target> --json
kdb refs <target> --count
```

## Output

```
$ kdb refs docs/hooks.md
tutorial.md:12:5  [React Hooks](docs/hooks.md)
index.md:8:3      [[hooks]]

$ kdb refs docs/hooks.md#useEffect
components.md:45:10  [useEffect](docs/hooks.md#useEffect)
patterns.md:22:1     [[hooks#useEffect]]

$ kdb refs docs/hooks.md --count
2
```

## Flags

- `--json` — structured output for programmatic consumption
- `--count` — just the number of references

## Implementation

- Build `VaultIndex`, query `file_inbound` and `heading_inbound` maps
- Parse the target argument to split file path from optional `#anchor`
- Output format: `source_file:line:col  raw_link_text`

## Scope

Markdown-only for now. Code import references (phase 3) will extend this later.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Refs` subcommand |
| `src/cmd.rs` | Add `cmd::refs()` entrypoint |
