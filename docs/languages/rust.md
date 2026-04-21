---
path: projects/kdb/docs/languages/rust.md
outline: |
  • Rust — Language Support              L11
    ◦ Import patterns                    L13
    ◦ Reference resolution coverage      L27
    ◦ Known gaps                         L46
    ◦ Workspace conventions              L53
---

# Rust — Language Support

## Import patterns

Rust uses `use` declarations to bring items into scope. Paths are rooted at the crate (`crate::`) or at an external dependency name.

```rust
use crate::foo::Bar;              // absolute crate path
use crate::foo::{Bar, Baz};      // grouped import
use crate::foo::Bar as B;        // aliased
use crate::foo::*;               // wildcard (glob)
pub use crate::inner::Foo;       // re-export
```

Module structure maps to the filesystem (`mod.rs` or `foo.rs` + `foo/`). `pub use` in `mod.rs` is the standard re-export pattern.

## Reference resolution coverage

Each category describes a way a symbol can be referenced after being imported. Status reflects what `refs -s` can resolve today.

| # | Category | Example | Status |
|---|---|---|---|
| R1 | Direct import | `use crate::foo::Bar; Bar::new()` | pass |
| R2 | Grouped import | `use crate::foo::{Bar, Baz};` | pass |
| R3 | Aliased import | `use crate::foo::Bar as B; B::new()` | fail — alias name in bindings, definition name in symbol_lookup |
| R4 | `pub use` re-export | `mod.rs` does `pub use inner::Foo;`, caller imports from mod | fail — re-export not followed |
| R5 | Wildcard import | `use crate::foo::*;` | fail — wildcard not expanded |
| R6 | Method on imported type | `let x = Bar::new(); x.method()` — ref to `Bar::method` | out of scope — requires type inference |
| R7 | Trait method call | `x.do_thing()` where `DoThing` trait is imported | out of scope — requires type inference |
| R8 | Macro-generated usage | `macro_rules!` that expands to use a symbol | out of scope — requires macro expansion |
| R9 | Type in signature | `fn f(x: Bar)` where Bar is imported | fail — `is_declaration_identifier` filters type in `parameter` parent (iss-0039.3) |
| R10 | Type in generic | `Vec<Bar>` where Bar is imported | pass |
| R11 | Module-qualified access | `use crate::event; event::Source` | pass |
| R12 | Grouped module-qualified access | `use crate::{event}; event::Source` | pass (but real-world needs 0039.5 re-export following for `mod.rs` → inner file) |

## Known gaps

- **`pub use` re-exports** — the most impactful gap. Very common in idiomatic Rust (`pub use` in `mod.rs`/`lib.rs`). Requires multi-hop resolution (iss-0028 v2).
- **Wildcard imports** — `use crate::*` makes it impossible to know which names are pulled in without type checking.
- **Trait method dispatch** — calling `.method()` on a trait object can't be resolved to a concrete impl without type information.
- **Macro expansion** — `macro_rules!` and proc macros generate code that tree-sitter can't see.

## Workspace conventions

- `Cargo.toml` workspace with `[workspace.members]`
- Crate roots: `src/lib.rs`, `src/main.rs`, or custom via `[lib] path`
- `mod` declarations map to filesystem paths
- Edition-dependent path behavior (2015 vs 2018+)
