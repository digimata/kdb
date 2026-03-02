> [docs](../../docs) · [specs](../specs)
> --------------------------------------
> docs/specs/refs.md
>
> # Spec: kdb refs                   L29
> ## CLI                             L35
> ### Markdown mode                  L37
> ### Code mode (-s)                 L42
> ## Workspace and paths             L55
> ## Target symbol identity          L61
> ## What counts as a reference      L70
> ### Must count                     L75
> ### Must not count                 L96
> ## Resolution model               L107
> ### Pipeline                      L113
> ### Import binding resolution     L130
> ### Qualified access              L140
> ### Re-export following           L149
> ### Go same-package resolution    L160
> ## Output contract                L167
> ### Sort order                    L179
> ### Text output                   L184
> ### JSON output                   L200
> ## Out of scope                   L205
> ## Unsupported patterns           L217
> ## Per-language coverage          L229
> --------------------------------------

# Spec: `kdb refs`

This document defines the intended behavior of `kdb refs`. The eval suite
(`tests/refs_eval.rs`) validates conformance. Known gaps are tracked in
[iss-0039](https://github.com/dremnik/kdb/issues/53).

## CLI

### Markdown mode

- `kdb refs <file>`: list inbound markdown references to `<file>`.
- `kdb refs <file>#<heading>`: list inbound markdown references to that heading.

### Code mode (`-s`)

```
kdb refs <def-file> -s <symbol>             # list references
kdb refs <def-file> -s <symbol> --count     # count only
kdb refs <def-file> -s <symbol> --json      # structured JSON
kdb refs <def-file> -s <symbol> -c <n>      # N lines of context (text only)
```

The symbol selector supports parent scoping: `-s Foo` matches all symbols
named `Foo`; `-s MyStruct::foo` matches only `foo` under parent `MyStruct`.
Trailing `()` is stripped (`foo()` → `foo`).

## Workspace and paths

- The workspace root is the directory containing `.kdb/`.
- `<def-file>` is a workspace-relative path to a supported code file.
- All output paths are workspace-relative.

## Target symbol identity

- `refs -s` targets all extracted symbols in `<def-file>` whose name matches
  `<symbol>` (and parent, if specified). If multiple definitions match,
  results are the union of references to all of them.
- The definition site is always included (`is_definition: true`).
- If `<symbol>` is not found in the parsed AST: error
  `symbol not found: <symbol> in <def-file>`.

## What counts as a reference

A reference is a syntactic identifier occurrence that resolves — through
import bindings or package scope — to the target symbol definition.

### Must count

- **Value usage**: `foo`, `foo(...)`, `Foo { ... }`.
- **Type usage**: annotations, generics, extends/implements, return types,
  parameter types.
- **Qualified access** where the qualifier is a bound module/package name:
  - Rust: `module::Foo`, `module::Foo::method()`.
  - TS/JS: `ns.foo`, `ns.Foo()`.
  - Python: `pkg.foo`, `pkg.Foo()`.
  - Go: `pkg.Foo`, `pkg.Foo{}`.
- **JSX/TSX**: component tags (`<Foo />`), expression identifiers (`{Foo}`),
  calls (`{doThing()}`).
- **Re-export statements** that explicitly name the symbol:
  - Rust: `pub use path::Foo;`.
  - TS/JS: `export { foo } from './x'`, `export { foo }`.
  - Python: `from .x import Foo` in `__init__.py`.
- **Aliased usage**: `import { Foo as F }; F()` counts as a reference to `Foo`.
- **Go same-package usage**: Go files in the same directory share a package
  namespace — `Foo()` in `b.go` is a reference to `Foo` defined in `a.go`,
  no import required.

### Must not count

- **Import specifiers**: `import { foo } from './x'`, `use path::Foo;`,
  `from x import foo`, `import "path"`. These are bindings, not usages.
- **Declaration identifiers**: the `foo` in `fn foo()`, `class Foo`,
  `def foo()`, `func Foo()`. The definition site is tracked separately
  via `is_definition`.
- **Field names**: struct field definitions, object keys, named parameters.
- **Comments, docstrings, string literals**.
- **Dynamic/runtime constructs**: `import()`, `getattr()`, reflection.

## Resolution model

`refs -s` does **name resolution** — tracing imported symbol names from
definition to usage across files. It does not do type inference, macro
expansion, or runtime analysis.

### Pipeline

The index is built in sequential phases:

1. **Load**: read source text for all code files in the import map.
2. **Extract symbols**: tree-sitter symbol extraction per file (declarations).
3. **Build symbol lookup**: per-file map of `name → [SymbolKey]`.
4. **Build re-export lookup**: per-file map of `name → [ReexportTarget]`
   from re-export statements matched against resolved imports.
5. **Seed definitions**: insert `is_definition = true` row for each symbol.
6. **Link usage refs**: for each file, build import bindings from its
   resolved imports, scan for identifier usages matching bound names,
   and insert usage rows.
7. **Go same-package refs**: for Go files grouped by directory, scan each
   file for usages of symbols defined in sibling files (no import needed).
8. **Normalize**: sort and deduplicate all ref rows.

### Import binding resolution

For each source file, import bindings are built from `ResolvedImport` data:

- **Direct names**: each imported name maps to its target file.
- **Aliases**: `import { Foo as F }` — `F` maps to target file, with
  `F → Foo` in the alias table so the definition name is used for lookup.
- **Namespace imports**: Go dot imports (`import . "pkg"`) expand all
  symbols from the target file into the local scope.

### Qualified access

When a usage is qualified (`module::Foo`, `ns.foo`, `pkg.Func`), the scanner
decomposes it into (binding_name, symbol_name). The binding name is matched
against import bindings; the symbol name is looked up in the target file.

Rust handles nested paths: `a::b::c` extracts the leftmost identifier as
the binding name.

### Re-export following

When a usage resolves to a file that doesn't define the symbol but
re-exports it, the chain is followed to the actual definition site.

- Uses a visited set for cycle detection (not a depth limit).
- Re-export patterns recognized:
  - Rust: `pub use inner::Foo;` in `mod.rs` / `lib.rs`.
  - TS/JS: `export { Foo } from './foo'` in barrel `index.ts`.
  - Python: `from .inner import Foo` in `__init__.py`.

### Go same-package resolution

Go files in the same directory belong to the same package and share a
namespace without imports. After import-based linking, a separate pass
groups Go files by parent directory and scans each file for usages of
symbols defined in sibling files.

## Output contract

Each result row:

| Field | Type | Description |
|---|---|---|
| `source_file` | path | Workspace-relative path to the file containing the reference |
| `line` | int | 1-based line number |
| `column` | int | 1-based column number |
| `snippet` | string | Trimmed source line at the reference location |
| `is_definition` | bool | `true` for the declaration site, `false` for usages |

### Sort order

Definition rows first, then usage rows sorted by `(source_file, line, column)`.
Duplicates (same file + line + column + snippet + is_definition) are removed.

### Text output

```
<file>:<line>:<column>  <snippet>
```

With `-c <n>`, each reference is rendered as a context block:

```
<file>:<line>:<column>
  <line-2> | preceding line
> <line-1> | matching line
  <line>   | following line
--
```

### JSON output

Array of `SymbolRef` objects with the fields above. Serialized with
`serde_json::to_string_pretty`.

## Out of scope

These require capabilities beyond syntactic name resolution and are
permanently excluded from `refs -s`:

| Category | Why | Examples |
|---|---|---|
| Type-based method dispatch | Requires type inference | `conn.execute()`, trait object calls |
| Macro expansion | Code invisible to tree-sitter | `macro_rules!`, proc macros, `cfg_if!` |
| Dynamic imports / reflection | Runtime constructs | `import()`, `getattr()`, `reflect` |
| Wildcard import expansion | Requires type checker to enumerate names | `use crate::*`, `from x import *` |

## Unsupported patterns

Patterns not yet handled by `kdb refs` (see [iss-0039](https://github.com/dremnik/kdb/issues/53) for history):

- **Compound resolution** (iss-0041): grouped import + directory module +
  re-export chain don't compose in a single pass. Requires multi-pass
  pipeline.
- **tsconfig path aliases** (iss-0039.8): `@/foo` mappings not resolved.
- **Member access on imported value** (iss-0039.14): `Foo.method()` where
  `Foo` is a named import.
- **Rust scoped imports** (iss-0039.12): `use` inside function bodies.

## Per-language coverage

Detailed per-language import patterns and coverage matrices:

- [Rust](../languages/rust.md)
- [TypeScript / JavaScript](../languages/typescript.md)
- [Python](../languages/python.md)
- [Go](../languages/go.md)
