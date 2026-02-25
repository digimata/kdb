# Python — Language Support

## Import patterns

Python uses `import` and `from ... import` with both absolute and relative paths. Package structure is defined by `__init__.py` files (or namespace packages without them).

```python
from foo import bar                  # direct import
import foo                           # module import (access via foo.bar)
from foo import bar as b             # aliased
from foo import *                    # wildcard
from .foo import bar                 # relative import
```

Re-exports happen through `__init__.py` and `__all__`:

```python
# __init__.py
from .core import Engine
from .utils import helper
__all__ = ['Engine', 'helper']
```

## Reference resolution coverage

| # | Category | Example | Status |
|---|---|---|---|
| P1 | Direct import | `from foo import bar; bar()` | fail — `from X import name` resolves name as submodule, not symbol (iss-0039.1) |
| P2 | Module import | `import foo; foo.bar()` | fail — namespace access not decomposed |
| P3 | Aliased import | `from foo import bar as b; b()` | fail — alias name in bindings, definition name in symbol_lookup |
| P4 | Wildcard import | `from foo import *; bar()` | fail — wildcard not expanded |
| P5 | `__all__` re-export | `__init__.py` re-exports via `__all__` | fail — re-export not followed |
| P6 | Relative import | `from .foo import bar` | fail — same symbol binding gap as P1 (iss-0039.1) |
| P7 | Decorator usage | `@bar` where `bar` is imported | fail — same symbol binding gap as P1 (iss-0039.1) |
| P8 | Type annotation | `def f(x: Bar)` where `Bar` is imported | fail — same symbol binding gap as P1 (iss-0039.1) |
| P9 | Class instantiation | `Bar()` where `Bar` is imported | fail — same symbol binding gap as P1 (iss-0039.1) |

## Known gaps

- **`__init__.py` re-exports** — very common pattern. `from mypackage import Foo` resolves to `mypackage/__init__.py`, but `Foo` is defined in `mypackage/core.py`. Same multi-hop problem as TS barrel files.
- **`__all__`** — controls what `import *` pulls in, and also signals the public API of a package.
- **Module-level access** — `import foo; foo.bar()` requires knowing that `bar` is an attribute of module `foo`, which we don't trace.
- **Wildcard imports** — can't determine which names are in scope without executing or type-checking.
- **Dynamic attribute access** — `getattr(obj, "bar")` is invisible to static analysis.

## Workspace conventions

- `setup.py` / `setup.cfg` / `pyproject.toml` for package definitions
- `src/` layout vs flat layout
- Virtual environments (`venv`, `conda`)
- Namespace packages (no `__init__.py`) in modern Python
- Relative imports within packages
