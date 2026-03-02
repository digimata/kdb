> ---------------------------------------------------
> docs/languages/typescript.md
>
> TypeScript / JavaScript — Language Support      L11
>   • Import patterns                             L13
>   • Reference resolution coverage               L36
>   • Known gaps                                  L52
>   • Workspace conventions                       L59
> ---------------------------------------------------

# TypeScript / JavaScript — Language Support

## Import patterns

TS/JS has the most varied import system of any language kdb supports. Named, default, namespace, and dynamic imports all coexist, plus CommonJS `require()`.

```typescript
import { foo } from './bar';           // named import
import foo from './bar';               // default import
import { foo as f } from './bar';      // aliased
import * as bar from './bar';          // namespace import
import type { Foo } from './bar';      // type-only import (TS)
const { foo } = require('./bar');      // CommonJS
const m = await import('./bar');       // dynamic import
```

Re-exports through barrel files (`index.ts`) are extremely common:

```typescript
// index.ts
export { foo } from './foo';
export { default as Bar } from './bar';
export * from './utils';
```

## Reference resolution coverage

| # | Category | Example | Status |
|---|---|---|---|
| T1 | Named import | `import { foo } from './bar'; foo()` | pass |
| T2 | Default import | `import foo from './bar'; foo()` | pass |
| T3 | Aliased import | `import { foo as f } from './bar'; f()` | fail — alias name in bindings, definition name in symbol_lookup |
| T4 | Namespace import | `import * as bar from './bar'; bar.foo()` | fail — namespace not decomposed |
| T5 | Barrel re-export | `index.ts` does `export { foo } from './inner'` | fail — re-export not followed |
| T6 | Default re-export | `export { default as foo } from './inner'` | fail — re-export not followed |
| T7 | Dynamic import | `const m = await import('./bar'); m.foo()` | out of scope — runtime construct |
| T8 | CommonJS require | `const { foo } = require('./bar')` | pass |
| T9 | Type import | `import type { Foo } from './bar'; let x: Foo` | pass |
| T10 | Destructured usage | `const { a, b } = foo()` where `foo` is imported | pass |
| T11 | JSX component | `<Foo />` where `Foo` is imported | fail — `jsx_identifier` not in `is_usage_identifier` (iss-0039.3) |

## Known gaps

- **Barrel files** — the biggest gap. Nearly every TS/JS project uses `index.ts` re-exports. `import { Foo } from './components'` resolves to `components/index.ts`, but `Foo` actually lives in `components/Foo.tsx`. Requires multi-hop re-export following (iss-0028 v2).
- **Namespace imports** — `import * as X` makes all access go through `X.name`, which we don't trace through.
- **Dynamic imports** — runtime `import()` returns a module object; property access on it is a reference.
- **Path resolution complexity** — `tsconfig.json` paths, `baseUrl`, `exports` field in `package.json`, `.js` extension in ESM imports pointing to `.ts` files.

## Workspace conventions

- `tsconfig.json` with `compilerOptions.paths` for path aliases
- `package.json` workspaces (monorepo)
- `node_modules` resolution (local → hoisted)
- File extensions: `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`
- Index files: `index.ts` / `index.js` as default module entry
