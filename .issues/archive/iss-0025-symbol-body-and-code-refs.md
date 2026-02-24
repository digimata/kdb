---
id: 25
title: Symbol body extraction (symbols -s)
status: proposed
priority: high
labels:
  - feat
  - symbols
---

# ISS-0025 :: Symbol body extraction (`symbols -s`)

## Intent

Add `-s <name>` flag to `kdb symbols` to print the full implementation body of a named symbol. Single-file operation, no index needed.

## CLI

```
kdb symbols <file>                    # list all symbols (current behavior)
kdb symbols <file> -s <name>          # print full implementation body
kdb symbols <file> -s <name> --json   # body + metadata as JSON
```

Examples:

```
$ kdb symbols src/api/router.ts -s handleRequest
export async function handleRequest(req: Request): Promise<Response> {
  const user = await authenticate(req);
  const route = matchRoute(req.url);
  return dispatch(route, user, req);
}

$ kdb symbols src/cmd.rs -s init
pub fn init(path: Option<PathBuf>) -> Result<()> {
    let start = match path { ... };
    ...
    Ok(())
}
```

Name resolution:
- Bare name: `-s init` matches function `init`
- Qualified: `-s Backend::new` matches method `new` on `Backend`
- Multiple matches: print all, separated by blank lines
- No match: exit with error

## How it works

Tree-sitter gives us the byte span of every declaration node. When `-s` is provided:

1. Parse the file, extract symbols (same as current)
2. Find the matching symbol(s) by name
3. Use the tree-sitter node's `start_byte..end_byte` to slice the source text
4. Print the slice

The node span covers the entire declaration including doc comments/decorators above it.

## Dependencies

- **iss-0024** (native symbol display): symbol model needs `start_byte..end_byte` spans

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `-s`/`--symbol` flag to `Symbols` subcommand |
| `src/cmd.rs` | Branch on `-s` presence in `symbols()` |
| `src/symbols/mod.rs` | Add `extract_symbol_body()` — find symbol node, return source slice |
| `src/symbols/render.rs` | Add body printing mode + JSON body output |
