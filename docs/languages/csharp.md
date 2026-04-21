---
path: projects/kdb/docs/languages/csharp.md
outline: |
  • C# — Language Support                L12
    ◦ Import patterns                    L14
    ◦ Reference resolution coverage      L34
    ◦ Known challenges                   L45
    ◦ Advantages                         L52
    ◦ Workspace conventions              L58
---

# C# — Language Support

## Import patterns

C# uses `using` directives to import namespaces. Unlike most other languages kdb supports, namespaces are decoupled from the filesystem — a file can declare any namespace regardless of its path.

```csharp
using System;                          // namespace import
using System.Collections.Generic;      // nested namespace
using static System.Math;              // static import (members in scope directly)
using Alias = Some.Long.Namespace;     // aliased namespace
using global::Some.Namespace;          // global qualifier (avoids ambiguity)
```

C# 10+ adds file-scoped and global usings:

```csharp
global using System.Linq;             // applies to all files in the project
```

No re-export mechanism — namespaces are flat containers of types.

## Reference resolution coverage

| # | Category | Example | Status |
|---|---|---|---|
| CS1 | Namespace import | `using Foo; Bar.Method()` | not implemented |
| CS2 | Nested namespace | `using Foo.Bar; Baz.Method()` | not implemented |
| CS3 | Static import | `using static System.Math; Sqrt(4)` | not implemented |
| CS4 | Aliased namespace | `using A = Foo.Bar; A.Baz()` | not implemented |
| CS5 | Global using | `global using System.Linq;` | not implemented |
| CS6 | Type usage | `var x = new Foo.Bar()` | not implemented |

## Known challenges

- **Namespace ≠ filesystem path** — the biggest difference from other supported languages. `namespace Foo.Bar` can appear in any `.cs` file regardless of directory. Import resolution needs to scan all files to build a namespace→type map.
- **Partial classes** — a single class can be split across multiple files. Symbol extraction needs to handle this.
- **Global usings** — apply across all files in a project, effectively implicit imports.
- **NuGet packages** — external dependencies resolved through `.csproj` package references.

## Advantages

- No re-exports or barrel files — types live in namespaces, accessed directly.
- No wildcard imports — `using Foo` brings the namespace into scope, not its members (except `using static`).
- Mature tree-sitter grammar (`tree-sitter-c-sharp`).

## Workspace conventions

- `.csproj` defines project, dependencies, and build settings
- `.sln` for multi-project solutions (workspaces)
- `bin/` and `obj/` for build output (should be ignored)
- `global.json` for SDK version pinning
- NuGet packages in `~/.nuget/packages` (global cache)
