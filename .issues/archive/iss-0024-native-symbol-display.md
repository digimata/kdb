---
id: 24
title: Language-native symbol display
status: proposed
priority: high
labels:
  - enhancement
  - symbols
  - fmt
---

# ISS-0024 :: Language-native symbol display

## Intent

Stop compressing all symbols through Rust-flavored syntax (`fn`, `struct`, etc.) and instead display symbols using their language-native keywords and modifiers. A TypeScript file should show `export async function run()`, not `fn run()`. This affects both `kdb symbols` CLI output and `kdb fmt` index headers.

## Current behavior

```
$ kdb symbols agent.ts
class Agent                L38
  fn Agent::constructor()  L52
  fn Agent::run()          L101
  fn Agent::stream()       L118
  fn Agent::prepare()      L129
  fn Agent::apply()        L196
```

Everything is `fn`. No visibility, no async, no getters, no properties.

## Desired behavior

```
$ kdb symbols agent.ts
export class Agent                   L38
  readonly kind                      L39
  readonly model                     L40
  constructor()                      L52
  get threads()                      L72
  async run()                        L101
  async *stream()                    L118
  private async prepare()            L129
  private apply()                    L196
```

## Design

### Symbol struct changes

Replace the current `SymbolKind` enum + `keyword_for_kind()` with a richer model:

```rust
pub struct Symbol {
    pub name: String,
    pub parent: Option<String>,
    pub kind: SymbolKind,       // keep for categorization/filtering
    pub display_kind: String,   // language-native keyword string, e.g. "export async function"
    pub line: usize,
    pub is_public: bool,
}
```

Each per-language extractor populates `display_kind` with the actual keyword chain from source. The render layer uses `display_kind` directly instead of mapping through `keyword_for_kind()`.

### New symbol kinds

Extend `SymbolKind` for filtering/categorization (not display):

```rust
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Class,
    Interface,
    Const,        // new
    Static,       // new
    Property,     // new (class fields, readonly properties)
    Getter,       // new
    Setter,       // new
    Module,       // new (Rust mod, TS namespace)
    Macro,        // new (Rust macro_rules!)
    Constructor,  // new
    Variable,     // new (module-level let/var, Go var)
}
```

## Per-language spec

### Rust

| Construct | display_kind | SymbolKind | Notes |
|---|---|---|---|
| `pub fn name()` | `"pub fn"` | Function | Preserve visibility: `pub`, `pub(crate)`, `pub(super)`, or none |
| `pub async fn name()` | `"pub async fn"` | Function | Modifier order: `pub` > `const`/`async` > `unsafe` > `extern` > `fn` |
| `const fn name()` | `"const fn"` | Function | |
| `unsafe fn name()` | `"unsafe fn"` | Function | |
| `fn name()` (in impl) | `"fn"` or `"pub fn"` | Method | Indented under parent |
| `pub struct Name` | `"pub struct"` | Struct | |
| `pub enum Name` | `"pub enum"` | Enum | |
| `pub trait Name` | `"pub trait"` | Trait | `pub unsafe trait` for unsafe traits |
| `pub type Name` | `"pub type"` | TypeAlias | |
| `pub const NAME` | `"pub const"` | Const | **New** |
| `static mut NAME` | `"static mut"` | Static | **New** |
| `pub mod name` | `"pub mod"` | Module | **New** |
| `macro_rules! name` | `"macro_rules!"` | Macro | **New** |

### TypeScript / JavaScript

| Construct | display_kind | SymbolKind | Notes |
|---|---|---|---|
| `function name()` | `"function"` | Function | |
| `export function name()` | `"export function"` | Function | |
| `export default function name()` | `"export default function"` | Function | |
| `async function name()` | `"async function"` | Function | |
| `export async function name()` | `"export async function"` | Function | |
| `function* name()` | `"function*"` | Function | Generator |
| `async function* name()` | `"async function*"` | Function | Async generator |
| `class Name` | `"class"` | Class | |
| `export class Name` | `"export class"` | Class | |
| `abstract class Name` | `"abstract class"` | Class | |
| `interface Name` | `"interface"` | Interface | |
| `export interface Name` | `"export interface"` | Interface | |
| `type Name` | `"type"` | TypeAlias | |
| `export type Name` | `"export type"` | TypeAlias | |
| `enum Name` | `"enum"` | Enum | |
| `const enum Name` | `"const enum"` | Enum | |
| `export const name` | `"export const"` | Const | Module-level only |
| `const name` | `"const"` | Const | Module-level only |
| `export let name` | `"export let"` | Variable | Module-level only |
| `constructor()` | `"constructor"` | Constructor | Class member |
| `methodName()` | `""` (none) | Method | Plain method, no keyword |
| `async methodName()` | `"async"` | Method | |
| `static methodName()` | `"static"` | Method | |
| `private methodName()` | `"private"` | Method | |
| `private async methodName()` | `"private async"` | Method | |
| `protected methodName()` | `"protected"` | Method | |
| `abstract methodName()` | `"abstract"` | Method | |
| `override methodName()` | `"override"` | Method | |
| `get name()` | `"get"` | Getter | |
| `set name()` | `"set"` | Setter | |
| `readonly name` | `"readonly"` | Property | Class field |
| `private readonly name` | `"private readonly"` | Property | |
| `static name` | `"static"` | Property | Static field |
| `#name` | `"#"` | Property | Private field (ES private) |
| `async *name()` | `"async *"` | Method | Async generator method |

### Python

| Construct | display_kind | SymbolKind | Notes |
|---|---|---|---|
| `def name()` | `"def"` | Function | Module-level |
| `async def name()` | `"async def"` | Function | |
| `class Name` | `"class"` | Class | |
| `def name(self)` | `"def"` | Method | Inside class |
| `async def name(self)` | `"async def"` | Method | |
| `def __init__(self)` | `"def"` | Constructor | Detected by name |
| `@property def name(self)` | `"@property def"` | Getter | |
| `@name.setter def name(self, v)` | `"@setter def"` | Setter | |
| `@staticmethod def name()` | `"@staticmethod def"` | Method | |
| `@classmethod def name(cls)` | `"@classmethod def"` | Method | |
| `@abstractmethod def name(self)` | `"@abstractmethod def"` | Method | |

### Go

| Construct | display_kind | SymbolKind | Notes |
|---|---|---|---|
| `func Name()` | `"func"` | Function | Exported if capitalized |
| `func name()` | `"func"` | Function | Unexported |
| `func (r *Type) Name()` | `"func"` | Method | Receiver shown in name: `Type.Name()` |
| `type Name struct` | `"type struct"` | Struct | |
| `type Name interface` | `"type interface"` | Interface | |
| `type Name = Other` | `"type"` | TypeAlias | |
| `const Name` | `"const"` | Const | **New** |
| `var Name` | `"var"` | Variable | **New** |

## Rendering

The render layer (`symbols/render.rs`) changes from:

```
{indent}{keyword_for_kind} {qualified_name}  L{line}
```

to:

```
{indent}{display_kind} {name}()  L{line}
```

Where `()` is appended only for callable symbols (Function, Method, Constructor, Getter, Setter).

Methods are indented under their parent by structure, so no need for `Agent::prepare()` qualification — just `private async prepare()` indented under `class Agent`.

## Scope

- **Extract**: declaration-level symbols + class/struct members. No locals, no function-body variables.
- **Affects**: `kdb symbols` output, `kdb fmt` index headers, future `kdb codemap` output.
- **Backward compat**: The `--json` output adds the `display_kind` field. `kind` field remains for filtering.

## Changes

| File | Change |
|---|---|
| `src/symbols/mod.rs` | Add `display_kind: String` to `Symbol`, extend `SymbolKind` |
| `src/symbols/rust.rs` | Populate `display_kind` with visibility + modifiers |
| `src/symbols/typescript.rs` | Populate `display_kind` with export/async/private/etc. |
| `src/symbols/python.rs` | Populate `display_kind` with def/async def/decorators |
| `src/symbols/go.rs` | Populate `display_kind` with func/type/const/var |
| `src/symbols/render.rs` | Use `display_kind` instead of `keyword_for_kind()` |
| `src/fmt/mod.rs` | Update `render_block()` to use new display format |
