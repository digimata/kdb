use kdb::index::ProjectIndex;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// tests/refs_eval.rs
//
// Correctness evaluation fixture suite for `refs -s`.
// Each test exercises one reference category from iss-0028.1.
//
// Convention:
//   - `cargo test refs_eval` — runs passing tests (what works)
//   - `cargo test refs_eval -- --ignored` — runs gap tests (what's broken)
//   - The ratio is the scorecard
// ---------------------------------------------------------------------------

fn write_file(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write fixture file");
}

fn write_root_config(root: &Path) {
    write_file(root, ".kdb/config.toml", "[project]\nname = \"fixture\"\n");
}

/// Build the index and collect symbol refs for `symbol` defined in `target_file`.
///
/// Returns (definition_count, usage_count) — the definition row plus all
/// non-definition rows.
fn eval_refs(files: &[(&str, &str)], target_file: &str, symbol: &str) -> (usize, usize) {
    let tmp = tempdir().expect("tempdir");
    write_root_config(tmp.path());
    for (path, content) in files {
        write_file(tmp.path(), path, content);
    }
    let pi = ProjectIndex::build_with_symbol_refs(tmp.path(), &[])
        .expect("build project index with symbol refs");
    let rows = kdb::index::refs::collect_symbol_refs(&pi.code, tmp.path(), target_file, symbol)
        .expect("collect symbol refs");

    let defs = rows.iter().filter(|r| r.is_definition).count();
    let usages = rows.iter().filter(|r| !r.is_definition).count();
    (defs, usages)
}

// ═══════════════════════════════════════════════════════════════════════════
// Rust (R1–R10)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn rust_r01_direct_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub fn foo() {}\n"),
            (
                "src/caller.rs",
                "use crate::target::foo;\npub fn run() {\n    foo();\n}\n",
            ),
        ],
        "src/target.rs",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from direct import");
}

#[test]
fn rust_r02_grouped_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub fn foo() {}\npub fn bar() {}\n"),
            (
                "src/caller.rs",
                "use crate::target::{foo, bar};\npub fn run() {\n    foo();\n    bar();\n}\n",
            ),
        ],
        "src/target.rs",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage of foo from grouped import");
}

#[test]
fn rust_r03_aliased_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub struct Foo;\n"),
            (
                "src/caller.rs",
                "use crate::target::Foo as F;\npub fn run() {\n    let _ = F;\n}\n",
            ),
        ],
        "src/target.rs",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via alias");
}

#[test]
fn rust_r04_pub_use_reexport() {
    let (defs, usages) = eval_refs(
        &[
            (
                "src/lib.rs",
                "pub mod inner;\npub mod facade;\npub mod caller;\n",
            ),
            ("src/inner.rs", "pub struct Foo;\n"),
            ("src/facade.rs", "pub use crate::inner::Foo;\n"),
            (
                "src/caller.rs",
                "use crate::facade::Foo;\npub fn run() {\n    let _ = Foo;\n}\n",
            ),
        ],
        "src/inner.rs",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via re-export");
}

#[test]
fn rust_r05_wildcard_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub fn foo() {}\n"),
            (
                "src/caller.rs",
                "use crate::target::*;\npub fn run() {\n    foo();\n}\n",
            ),
        ],
        "src/target.rs",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via wildcard import");
}

#[test]
fn rust_r09_type_in_signature() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub struct Bar;\n"),
            (
                "src/caller.rs",
                "use crate::target::Bar;\npub fn f(_x: Bar) {}\n",
            ),
        ],
        "src/target.rs",
        "Bar",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as type in fn signature");
}

#[test]
fn rust_r10_type_in_generic() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub struct Bar;\n"),
            (
                "src/caller.rs",
                "use crate::target::Bar;\npub fn f() -> Vec<Bar> { vec![] }\n",
            ),
        ],
        "src/target.rs",
        "Bar",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as type in generic");
}

// ═══════════════════════════════════════════════════════════════════════════
// TypeScript / JavaScript (T1–T11)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn tsjs_t01_named_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/target.ts", "export function foo() {}\n"),
            ("src/caller.ts", "import { foo } from './target';\nfoo();\n"),
        ],
        "src/target.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from named import");
}

#[test]
fn tsjs_t02_default_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/target.ts", "export default function Foo() {}\n"),
            ("src/caller.ts", "import Foo from './target';\nFoo();\n"),
        ],
        "src/target.ts",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from default import");
}

#[test]
fn tsjs_t03_aliased_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/target.ts", "export function foo() {}\n"),
            (
                "src/caller.ts",
                "import { foo as f } from './target';\nf();\n",
            ),
        ],
        "src/target.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via alias");
}

#[test]
fn tsjs_t04_namespace_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/target.ts", "export function foo() {}\n"),
            (
                "src/caller.ts",
                "import * as bar from './target';\nbar.foo();\n",
            ),
        ],
        "src/target.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via namespace import");
}

#[test]
fn tsjs_t05_barrel_reexport() {
    let (defs, usages) = eval_refs(
        &[
            ("src/inner.ts", "export function foo() {}\n"),
            ("src/index.ts", "export { foo } from './inner';\n"),
            ("src/caller.ts", "import { foo } from './index';\nfoo();\n"),
        ],
        "src/inner.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via barrel re-export");
}

#[test]
fn tsjs_t06_default_reexport() {
    let (defs, usages) = eval_refs(
        &[
            ("src/inner.ts", "export default function foo() {}\n"),
            (
                "src/index.ts",
                "export { default as foo } from './inner';\n",
            ),
            ("src/caller.ts", "import { foo } from './index';\nfoo();\n"),
        ],
        "src/inner.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via default re-export");
}

#[test]
fn tsjs_t08_commonjs_require() {
    let (defs, usages) = eval_refs(
        &[
            (
                "src/target.js",
                "function foo() {}\nmodule.exports = { foo };\n",
            ),
            (
                "src/caller.js",
                "const { foo } = require('./target');\nfoo();\n",
            ),
        ],
        "src/target.js",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from require");
}

#[test]
fn tsjs_t09_type_import() {
    let (defs, usages) = eval_refs(
        &[
            ("src/target.ts", "export interface Foo { x: number }\n"),
            (
                "src/caller.ts",
                "import type { Foo } from './target';\nconst x: Foo = { x: 1 };\n",
            ),
        ],
        "src/target.ts",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from type import");
}

#[test]
fn tsjs_t10_destructured_usage() {
    let (defs, usages) = eval_refs(
        &[
            (
                "src/target.ts",
                "export function foo() { return { a: 1, b: 2 }; }\n",
            ),
            (
                "src/caller.ts",
                "import { foo } from './target';\nconst { a, b } = foo();\n",
            ),
        ],
        "src/target.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage of foo (destructured result)");
}

#[test]
fn tsjs_t11_jsx_component() {
    let (defs, usages) = eval_refs(
        &[
            ("src/target.tsx", "export function Foo() { return null; }\n"),
            (
                "src/caller.tsx",
                "import { Foo } from './target';\nexport default function App() { return <Foo />; }\n",
            ),
        ],
        "src/target.tsx",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as JSX component");
}

// ═══════════════════════════════════════════════════════════════════════════
// Python (P1–P9)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn python_p01_direct_import() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "def foo():\n    pass\n"),
            ("caller.py", "from target import foo\nfoo()\n"),
        ],
        "target.py",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from direct import");
}

#[test]
fn python_p02_module_import() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "def foo():\n    pass\n"),
            ("caller.py", "import target\ntarget.foo()\n"),
        ],
        "target.py",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via module namespace");
}

#[test]
fn python_p03_aliased_import() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "def foo():\n    pass\n"),
            ("caller.py", "from target import foo as f\nf()\n"),
        ],
        "target.py",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via alias");
}

#[test]
fn python_p04_wildcard_import() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "def foo():\n    pass\n"),
            ("caller.py", "from target import *\nfoo()\n"),
        ],
        "target.py",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via wildcard import");
}

#[test]
fn python_p04b_wildcard_all_filtering() {
    // `__all__` restricts what `*` imports — `bar` is NOT in `__all__`
    let (defs, usages) = eval_refs(
        &[
            (
                "target.py",
                "def foo():\n    pass\ndef bar():\n    pass\n__all__ = ['foo']\n",
            ),
            ("caller.py", "from target import *\nfoo()\n"),
        ],
        "target.py",
        "bar",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 0, "bar is excluded by __all__, no wildcard usage");
}

#[test]
fn python_p05_all_reexport() {
    let (defs, usages) = eval_refs(
        &[
            (
                "pkg/__init__.py",
                "from .inner import foo\n__all__ = ['foo']\n",
            ),
            ("pkg/inner.py", "def foo():\n    pass\n"),
            ("caller.py", "from pkg import foo\nfoo()\n"),
        ],
        "pkg/inner.py",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via __all__ re-export");
}

#[test]
fn python_p06_relative_import() {
    let (defs, usages) = eval_refs(
        &[
            ("pkg/__init__.py", ""),
            ("pkg/target.py", "def foo():\n    pass\n"),
            ("pkg/caller.py", "from .target import foo\nfoo()\n"),
        ],
        "pkg/target.py",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from relative import");
}

#[test]
fn python_p07_decorator_usage() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "def decorator(f):\n    return f\n"),
            (
                "caller.py",
                "from target import decorator\n@decorator\ndef run():\n    pass\n",
            ),
        ],
        "target.py",
        "decorator",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as decorator");
}

#[test]
fn python_p08_type_annotation() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "class Bar:\n    pass\n"),
            (
                "caller.py",
                "from target import Bar\ndef f(x: Bar) -> None:\n    pass\n",
            ),
        ],
        "target.py",
        "Bar",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as type annotation");
}

#[test]
fn python_p09_class_instantiation() {
    let (defs, usages) = eval_refs(
        &[
            ("target.py", "class Bar:\n    pass\n"),
            ("caller.py", "from target import Bar\nx = Bar()\n"),
        ],
        "target.py",
        "Bar",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage from class instantiation");
}

// ═══════════════════════════════════════════════════════════════════════════
// Go (G1–G7)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn go_g01_package_import() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            ("pkg/target.go", "package pkg\n\nfunc Foo() {}\n"),
            (
                "cmd/main.go",
                concat!(
                    "package main\n\n",
                    "import \"example.com/proj/pkg\"\n\n",
                    "func main() {\n\tpkg.Foo()\n}\n",
                ),
            ),
        ],
        "pkg/target.go",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via package import");
}

#[test]
fn go_g02_aliased_import() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            ("pkg/target.go", "package pkg\n\nfunc Foo() {}\n"),
            (
                "cmd/main.go",
                concat!(
                    "package main\n\n",
                    "import p \"example.com/proj/pkg\"\n\n",
                    "func main() {\n\tp.Foo()\n}\n",
                ),
            ),
        ],
        "pkg/target.go",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via aliased import");
}

#[test]
fn go_g03_dot_import() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            ("pkg/target.go", "package pkg\n\nfunc Foo() {}\n"),
            (
                "cmd/main.go",
                concat!(
                    "package main\n\n",
                    "import . \"example.com/proj/pkg\"\n\n",
                    "func main() {\n\tFoo()\n}\n",
                ),
            ),
        ],
        "pkg/target.go",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via dot import");
}

#[test]
fn go_g06_type_usage() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            ("pkg/target.go", "package pkg\n\ntype Foo struct{}\n"),
            (
                "cmd/main.go",
                concat!(
                    "package main\n\n",
                    "import \"example.com/proj/pkg\"\n\n",
                    "func main() {\n\tvar _ pkg.Foo\n}\n",
                ),
            ),
        ],
        "pkg/target.go",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as type");
}

#[test]
fn go_g07_composite_literal() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            (
                "pkg/target.go",
                "package pkg\n\ntype Foo struct {\n\tX int\n}\n",
            ),
            (
                "cmd/main.go",
                concat!(
                    "package main\n\n",
                    "import \"example.com/proj/pkg\"\n\n",
                    "func main() {\n\t_ = pkg.Foo{X: 1}\n}\n",
                ),
            ),
        ],
        "pkg/target.go",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as composite literal");
}
