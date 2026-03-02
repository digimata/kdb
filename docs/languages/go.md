> [docs](../../docs) · [languages](../languages)
> ---------------------------------------
> docs/languages/go.md
>
> # Go — Language Support             L13
> ## Import patterns                  L15
> ## Reference resolution coverage    L28
> ## Known gaps                       L40
> ## Advantages                       L46
> ## Workspace conventions            L56
> ---------------------------------------

# Go — Language Support

## Import patterns

Go imports are package-level. All exported names from a package are accessed through the package qualifier. The import path maps to a directory, and all `.go` files in that directory are part of the same package.

```go
import "pkg"                         // standard import, access via pkg.Foo
import p "pkg"                       // aliased import, access via p.Foo
import . "pkg"                       // dot import, access Foo directly (rare)
import _ "pkg"                       // side-effect import (init only)
```

Go has no re-export mechanism — every exported name lives in exactly one package. This makes import resolution simpler than Rust/TS/Python.

## Reference resolution coverage

| # | Category | Example | Status |
|---|---|---|---|
| G1 | Package import | `import "pkg"; pkg.Foo()` | fail — `import_names` returns pkg alias, not symbol names (iss-0039.2) |
| G2 | Aliased import | `import p "pkg"; p.Foo()` | fail — alias name in bindings, definition name in symbol_lookup |
| G3 | Dot import | `import . "pkg"; Foo()` | fail — dot import not expanded |
| G4 | Interface method | calling a method defined on an interface | out of scope — requires type inference |
| G5 | Embedded struct | promoted method from embedded struct | out of scope — requires type inference |
| G6 | Type usage | `var x pkg.Foo` | fail — same namespace gap as G1 (iss-0039.2) |
| G7 | Composite literal | `pkg.Foo{field: val}` | fail — same namespace gap as G1 (iss-0039.2) |

## Known gaps

- **Interface method dispatch** — calling `reader.Read()` can't be resolved to a specific implementation without type information.
- **Embedded struct promotion** — methods promoted from an embedded struct aren't directly visible as imports.
- **Dot imports** — `import . "pkg"` brings all exported names into scope without qualification, similar to wildcard imports in other languages.

## Advantages

Go's import model is the simplest of the four languages:
- No re-exports — every symbol has exactly one home
- No wildcards (dot imports are very rare in practice)
- Package path maps directly to filesystem directory
- All files in a directory are one package

This means Go should have the highest natural recall once basic qualified-name resolution works.

## Workspace conventions

- `go.mod` defines the module path
- `go.work` for multi-module workspaces
- `GOPATH` (legacy) vs module mode
- `internal/` packages restrict import visibility
- `vendor/` for vendored dependencies
- Package name = directory name (by convention)
