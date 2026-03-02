---
id: 40
title: "C# language support"
status: proposed
priority: medium
labels:
  - feat
  - lang
path: kdb/.issues/iss-0040-csharp-language-support.md
outline: |
  • ISS-0040 :: C# language support      L18
    ◦ Intent                             L20
    ◦ Import patterns                    L24
    ◦ Scope                              L42
    ◦ Notes                              L51
---

# ISS-0040 :: C# language support

## Intent

Add C# as a supported language for symbol extraction, import resolution, code formatting, and `refs -s`.

## Import patterns

C# uses `using` directives with namespace-level imports. No wildcard or re-export complexity.

```csharp
using System;                          // namespace import
using System.Collections.Generic;      // nested namespace
using static System.Math;              // static import (bring members into scope)
using Alias = Some.Long.Namespace;     // aliased namespace
using global::Some.Namespace;          // global qualifier
```

Project structure:
- `.csproj` defines the project and dependencies
- `.sln` for multi-project solutions
- Namespaces are independent of file paths (unlike most other languages)
- `global using` directives (C# 10+) apply across all files in a project

## Scope

- [ ] `CodeLanguage::CSharp` variant + `from_path` for `.cs` files
- [ ] Tree-sitter grammar: `tree-sitter-c-sharp`
- [ ] Symbol extraction: classes, structs, interfaces, enums, methods, properties, fields
- [ ] Import resolution: `using` directives → namespace → assembly/project mapping
- [ ] `refs -s` support: usage scanning with C# identifier kinds
- [ ] `fmt` support: comment prefix (`//`) + preamble detection (using block)

## Notes

- C# namespaces don't map to filesystem paths — a file can declare any namespace regardless of location. This makes import resolution different from Rust/Go/Python where the path is the namespace.
- NuGet packages for external dependencies.
- `tree-sitter-c-sharp` grammar is mature and widely used.
