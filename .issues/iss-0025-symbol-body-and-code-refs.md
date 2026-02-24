---
id: 25
title: Symbol body extraction and code symbol references
status: proposed
priority: high
labels:
  - feat
  - symbols
  - refs
---

# ISS-0025 :: Symbol body extraction and code symbol references

## Intent

Two enhancements that complete the agent navigation loop: (1) extract the full implementation body of a named symbol, and (2) find references to a code symbol across the project. These follow the rg design philosophy — one command, flags shape the output.

## Design principles

- No subcommands. `kdb symbols` and `kdb refs` each stay as one verb.
- `-s <name>` is the consistent flag across both commands meaning "I'm talking about a symbol, not the whole file."
- When `-s` narrows to a specific symbol in `kdb symbols`, the full body is printed (not just the listing line). If you wanted the listing, you'd query the whole file.
- Output is plain text by default, `--json` for structured.

## kdb symbols — enhanced

### Current behavior (unchanged)

```
kdb symbols <file>                    # list all symbols in file
kdb symbols <file> --json             # list as JSON
kdb symbols <file> --public           # public/exported only
kdb symbols <file> -k function        # filter by kind
```

### New: symbol body extraction

```
kdb symbols <file> -s <name>          # print full implementation body
kdb symbols <file> -s <name> --json   # body + metadata as JSON
```

`-s` / `--symbol` selects a symbol by name. When a symbol is selected, the output is the source text of the entire declaration (from first line to closing brace/dedent), printed verbatim.

#### Examples

```
$ kdb symbols src/api/router.ts -s handleRequest
export async function handleRequest(req: Request): Promise<Response> {
  const user = await authenticate(req);
  const route = matchRoute(req.url);
  return dispatch(route, user, req);
}

$ kdb symbols src/api/router.ts -s handleRequest --json
{
  "name": "handleRequest",
  "kind": "function",
  "display_kind": "export async function",
  "file": "src/api/router.ts",
  "line": 45,
  "end_line": 50,
  "public": true,
  "body": "export async function handleRequest(req: Request)..."
}

$ kdb symbols src/cmd.rs -s init
pub fn init(path: Option<PathBuf>) -> Result<()> {
    let start = match path {
        ...
    };
    ...
    Ok(())
}
```

#### Name resolution

- Bare name: `kdb symbols file.rs -s init` — matches function `init`
- Qualified name: `kdb symbols file.rs -s Backend::new` — matches method `new` on `Backend`
- If multiple symbols match, print all of them separated by blank lines
- If no match, exit with error

#### Implementation

Tree-sitter gives us the byte span of every declaration node. When `-s` is provided:

1. Parse the file, extract symbols (same as current)
2. Find the matching symbol(s) by name
3. Use the tree-sitter node's `start_byte..end_byte` to slice the source text
4. Print the slice

The node span covers the entire declaration including doc comments/decorators above it (use the `decorated_parent_or_self` pattern we already have for Python, extend to Rust `#[attributes]` and TS decorators).

## kdb refs — enhanced

### Current behavior (unchanged)

```
kdb refs <file>                       # markdown: who links to this file?
kdb refs <file>#<heading>             # markdown: who links to this heading?
kdb refs <target> --json              # structured output
kdb refs <target> --count             # just the count
```

### New: code symbol references

```
kdb refs <file> -s <name>             # find all references to this symbol
kdb refs <file> -s <name> --json
kdb refs <file> -s <name> --count
```

#### Examples

```
$ kdb refs src/root.rs -s find_root
src/cmd.rs:74:18           root::find_root(&start)
src/cmd.rs:92:18           root::find_root(&file_abs)
src/cmd.rs:147:18          root::find_root(&file_abs)
src/cmd.rs:211:18          root::find_root(&start)
src/fmt/mod.rs:42:18       root::find_root(&start)
tests/root.rs:15:10        find_root(&dir)

$ kdb refs src/root.rs -s find_root --count
6
```

#### Output format

Same as current refs: `file:line:col  context_snippet`

The context snippet shows the line containing the reference, trimmed.

#### Implementation

This is the expensive one — requires parsing every file in the project:

1. Resolve the target symbol's canonical name from the source file
2. Walk all supported code files in the project
3. For each file, parse with tree-sitter
4. Walk the AST looking for identifier nodes matching the symbol name
5. Filter out the definition itself (only report usages)
6. Report file:line:col for each match

**Phase 1 (simple):** Text-based search scoped to code files, similar to `rg` but using kdb's ignore patterns. Fast, some false positives.

**Phase 2 (semantic):** Tree-sitter based. Parse each file, walk identifiers, match by name and context. Slower but precise — can distinguish `root::find_root` from a local variable named `find_root`.

Start with phase 1. It's what agents need — speed over precision. They can read the context lines to filter.

## Relationship to existing commands

- `kdb symbols <file>` (iss-0018) — already implemented, this extends it with `-s`
- `kdb refs <target>` (iss-0019) — already implemented for markdown, this extends with `-s` for code
- `kdb deps <file>` (iss-0020) — outbound deps, complementary (deps = what I reach for, refs = who reaches for me)
- Native display (iss-0024) — `-s` output uses the native display format for the header line in `--json` mode

## Agent workflow

```
kdb tree                                    # orient
kdb symbols src/api/router.ts               # list symbols
kdb symbols src/api/router.ts -s handle     # read the implementation
kdb refs src/api/router.ts -s handle        # find who calls it
# ... make changes ...
kdb refs src/middleware/ratelimit.ts -s rateLimit  # verify wiring
```

Four commands, consistent flags, rg-style ergonomics.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `-s`/`--symbol` flag to `Symbols` and `Refs` subcommands |
| `src/cmd.rs` | Branch on `-s` presence in `symbols()` and `refs()` |
| `src/symbols/mod.rs` | Add `extract_symbol_body()` — find symbol node, return source slice |
| `src/symbols/render.rs` | Add body printing mode |
| `src/index/refs.rs` | Add `collect_code_refs()` — project-wide text/AST search for symbol usages |
