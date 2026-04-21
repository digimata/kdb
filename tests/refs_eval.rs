use kdb::index::WorkspaceIndex;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

// ------------------------------------------------------
// projects/kdb/tests/refs_eval.rs
//
// fn write_file()                                    L61
// fn write_root_config()                             L69
// fn eval_refs()                                     L77
// fn rust_r01_direct_import()                        L98
// fn rust_r02_grouped_import()                      L116
// fn rust_r03_aliased_import()                      L134
// fn rust_r04_pub_use_reexport()                    L152
// fn rust_r05_wildcard_import()                     L174
// fn rust_r09_type_in_signature()                   L192
// fn rust_r10_type_in_generic()                     L210
// fn rust_r11_module_qualified_access()             L228
// fn rust_r12_grouped_module_qualified_access()     L250
// fn rust_r13_multi_item_brace_group()              L275
// fn rust_r14_multi_hop_reexport()                  L304
// fn tsjs_t01_named_import()                        L334
// fn tsjs_t02_default_import()                      L348
// fn tsjs_t03_aliased_import()                      L362
// fn tsjs_t04_namespace_import()                    L379
// fn tsjs_t05_barrel_reexport()                     L396
// fn tsjs_t06_default_reexport()                    L414
// fn tsjs_t08_commonjs_require()                    L435
// fn tsjs_t09_type_import()                         L455
// fn tsjs_t10_destructured_usage()                  L472
// fn tsjs_t11_jsx_component()                       L492
// fn tsjs_t12_member_access_named_import()          L509
// fn python_p01_direct_import()                     L536
// fn python_p02_module_import()                     L550
// fn python_p03_aliased_import()                    L564
// fn python_p04_wildcard_import()                   L578
// fn python_p04b_wildcard_all_filtering()           L592
// fn python_p05_all_reexport()                      L610
// fn python_p06_relative_import()                   L631
// fn python_p07_decorator_usage()                   L646
// fn python_p08_type_annotation()                   L663
// fn python_p09_class_instantiation()               L680
// fn go_g01_package_import()                        L698
// fn go_g02_aliased_import()                        L720
// fn go_g03_dot_import()                            L742
// fn go_g06_type_usage()                            L764
// fn go_g07_composite_literal()                     L786
// fn go_g08_same_package()                          L811
// fn go_g09_same_package_type()                     L829
// fn tsjs_t11b_jsx_expression_identifier()          L854
// fn tsjs_t11c_jsx_call_inside_expression()         L875
// fn tsjs_t11d_jsx_opening_closing_tags()           L896
// fn tsjs_t11e_jsx_attribute_value()                L917
// fn tsjs_t11f_sonner_structure()                   L938
// fn tsjs_t11g_arrow_function_export_jsx()          L990
// fn tsjs_t11h_multi_import_from_same_file()       L1011
// fn tsjs_t11i_multi_import_no_jsx()               L1032
// ------------------------------------------------------

fn write_file(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write fixture file");
}

fn write_root_config(root: &Path) {
    write_file(root, ".kdb/config.toml", "[workspace]\nname = \"fixture\"\n");
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
    let pi = WorkspaceIndex::build_with_symbol_refs(tmp.path(), &[])
        .expect("build project index with symbol refs");
    let rows = kdb::index::refs::collect_symbol_refs(&pi.code, tmp.path(), target_file, symbol)
        .expect("collect symbol refs");

    let defs = rows.iter().filter(|r| r.is_definition).count();
    let usages = rows.iter().filter(|r| !r.is_definition).count();
    (defs, usages)
}

// ═══════════════════════════════════════════════════════════════════════════
// Rust (R1–R14)
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
    assert_eq!(usages, 2, "expected 2 usages: caller + re-export in facade");
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

#[test]
fn rust_r11_module_qualified_access() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod target;\npub mod caller;\n"),
            ("src/target.rs", "pub trait Source {}\n"),
            (
                "src/caller.rs",
                concat!(
                    "use crate::target;\n",
                    "pub struct Foo;\n",
                    "impl target::Source for Foo {}\n",
                ),
            ),
        ],
        "src/target.rs",
        "Source",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage via module-qualified path");
}

#[test]
fn rust_r12_grouped_module_qualified_access() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod event;\npub mod net;\n"),
            ("src/event.rs", "pub trait Source {}\n"),
            (
                "src/net.rs",
                concat!(
                    "use crate::{event};\n",
                    "pub struct UdpSocket;\n",
                    "impl event::Source for UdpSocket {}\n",
                ),
            ),
        ],
        "src/event.rs",
        "Source",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(
        usages, 1,
        "expected 1 usage via grouped module-qualified path"
    );
}

#[test]
fn rust_r13_multi_item_brace_group() {
    let (defs, usages) = eval_refs(
        &[
            ("src/lib.rs", "pub mod event;\npub mod net;\n"),
            (
                "src/event/mod.rs",
                "mod source;\npub use self::source::Source;\n",
            ),
            ("src/event/source.rs", "pub trait Source {}\n"),
            (
                "src/net.rs",
                concat!(
                    "use crate::{event, Token};\n",
                    "pub struct Foo;\n",
                    "impl event::Source for Foo {}\n",
                ),
            ),
        ],
        "src/event/source.rs",
        "Source",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(
        usages, 2,
        "expected 2 usages: qualified access in net.rs + re-export in mod.rs"
    );
}

#[test]
fn rust_r14_multi_hop_reexport() {
    let (defs, usages) = eval_refs(
        &[
            (
                "src/lib.rs",
                "pub mod inner;\npub mod mid;\npub mod facade;\npub mod caller;\n",
            ),
            ("src/inner.rs", "pub struct Foo;\n"),
            ("src/mid.rs", "pub use crate::inner::Foo;\n"),
            ("src/facade.rs", "pub use crate::mid::Foo;\n"),
            (
                "src/caller.rs",
                "use crate::facade::Foo;\npub fn run() {\n    let _ = Foo;\n}\n",
            ),
        ],
        "src/inner.rs",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(
        usages, 3,
        "expected 3 usages: caller + mid re-export + facade re-export"
    );
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
    assert_eq!(
        usages, 2,
        "expected 2 usages: caller + barrel re-export in index"
    );
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
    assert_eq!(
        usages, 2,
        "expected 2 usages: caller + default re-export in index"
    );
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

#[test]
fn tsjs_t12_member_access_named_import() {
    let (defs, usages) = eval_refs(
        &[
            (
                "src/state.ts",
                "export const ToastState = { subscribe() {}, dismiss() {} };\n",
            ),
            (
                "src/caller.ts",
                "import { ToastState } from './state';\nToastState.subscribe();\nToastState.dismiss();\n",
            ),
        ],
        "src/state.ts",
        "ToastState",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(
        usages, 2,
        "expected 2 usages of ToastState via member access"
    );
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
    assert_eq!(
        usages, 2,
        "expected 2 usages: caller + __init__.py re-export"
    );
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

#[test]
fn go_g08_same_package() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            ("pkg/target.go", "package pkg\n\nfunc Foo() {}\n"),
            (
                "pkg/other.go",
                concat!("package pkg\n\n", "func Bar() {\n\tFoo()\n}\n",),
            ),
        ],
        "pkg/target.go",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 same-package usage");
}

#[test]
fn go_g09_same_package_type() {
    let (defs, usages) = eval_refs(
        &[
            ("go.mod", "module example.com/proj\n\ngo 1.21\n"),
            (
                "pkg/target.go",
                "package pkg\n\ntype Kubelet struct {\n\tName string\n}\n",
            ),
            (
                "pkg/other.go",
                concat!("package pkg\n\n", "func (kl *Kubelet) GetPods() {}\n",),
            ),
        ],
        "pkg/target.go",
        "Kubelet",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 same-package type usage");
}

// ═══════════════════════════════════════════════════════════════════════════
// TSX/JSX real-world patterns (iss-0039.10)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn tsjs_t11b_jsx_expression_identifier() {
    // {icons?.close ?? CloseIcon} — identifier inside JSX expression container
    let (defs, usages) = eval_refs(
        &[
            ("src/assets.tsx", "export const CloseIcon = () => null;\n"),
            (
                "src/caller.tsx",
                concat!(
                    "import { CloseIcon } from './assets';\n",
                    "export function App() { return <div>{icons ?? CloseIcon}</div>; }\n",
                ),
            ),
        ],
        "src/assets.tsx",
        "CloseIcon",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage in JSX expression");
}

#[test]
fn tsjs_t11c_jsx_call_inside_expression() {
    // {isAction(toast)} — function call inside JSX expression container
    let (defs, usages) = eval_refs(
        &[
            ("src/types.ts", "export function isAction(x: unknown): boolean { return true; }\n"),
            (
                "src/caller.tsx",
                concat!(
                    "import { isAction } from './types';\n",
                    "export function App() { return <div>{isAction(toast) ? 'yes' : 'no'}</div>; }\n",
                ),
            ),
        ],
        "src/types.ts",
        "isAction",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage as call in JSX expression");
}

#[test]
fn tsjs_t11d_jsx_opening_closing_tags() {
    // <Loader>child</Loader> — both opening and closing tags
    let (defs, usages) = eval_refs(
        &[
            ("src/assets.tsx", "export function Loader() { return null; }\n"),
            (
                "src/caller.tsx",
                concat!(
                    "import { Loader } from './assets';\n",
                    "export function App() { return <Loader>child</Loader>; }\n",
                ),
            ),
        ],
        "src/assets.tsx",
        "Loader",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 2, "expected 2 usages (opening + closing JSX tags)");
}

#[test]
fn tsjs_t11e_jsx_attribute_value() {
    // <Foo bar={Baz} /> — identifier in attribute value
    let (defs, usages) = eval_refs(
        &[
            ("src/target.tsx", "export const Baz = 42;\n"),
            (
                "src/caller.tsx",
                concat!(
                    "import { Baz } from './target';\n",
                    "export function App() { return <div data-val={Baz} />; }\n",
                ),
            ),
        ],
        "src/target.tsx",
        "Baz",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage in JSX attribute");
}

#[test]
fn tsjs_t11f_sonner_structure() {
    // Replicate sonner's exact structure: multi-import from a file with JSX usage
    let (defs, usages) = eval_refs(
        &[
            (
                "src/assets.tsx",
                concat!(
                    "import React from 'react';\n",
                    "export const getAsset = (type: string) => null;\n",
                    "export const Loader = ({ visible }: { visible: boolean }) => {\n",
                    "  return <div className={visible ? 'show' : 'hide'}>loading</div>;\n",
                    "};\n",
                    "export const CloseIcon = () => <svg />;\n",
                ),
            ),
            (
                "src/types.ts",
                concat!(
                    "export type Action = { label: string };\n",
                    "export function isAction(x: unknown): x is Action { return true; }\n",
                ),
            ),
            (
                "src/index.tsx",
                concat!(
                    "'use client';\n",
                    "import React from 'react';\n",
                    "import { CloseIcon, getAsset, Loader } from './assets';\n",
                    "import { isAction } from './types';\n",
                    "\n",
                    "export function Toaster() {\n",
                    "  const toast = { type: 'loading', cancel: { label: 'x' } };\n",
                    "  const icons = { close: null as any };\n",
                    "  return (\n",
                    "    <div>\n",
                    "      <Loader visible={toast.type === 'loading'} />\n",
                    "      {icons?.close ?? CloseIcon}\n",
                    "      {isAction(toast.cancel) ? <span /> : null}\n",
                    "    </div>\n",
                    "  );\n",
                    "}\n",
                ),
            ),
        ],
        "src/assets.tsx",
        "Loader",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage of Loader as JSX component");
}

#[test]
fn tsjs_t11g_arrow_function_export_jsx() {
    // Arrow function export with JSX in body — does the scanner still find usages?
    let (defs, usages) = eval_refs(
        &[
            (
                "src/target.tsx",
                "export const Loader = ({ visible }: { visible: boolean }) => {\n  return <div>loading</div>;\n};\n",
            ),
            (
                "src/caller.tsx",
                "import { Loader } from './target';\nexport function App() { return <Loader visible={true} />; }\n",
            ),
        ],
        "src/target.tsx",
        "Loader",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage of arrow-exported Loader");
}

#[test]
fn tsjs_t11h_multi_import_from_same_file() {
    // Multiple named imports from the same file
    let (defs, usages) = eval_refs(
        &[
            (
                "src/target.tsx",
                "export function Foo() { return null; }\nexport function Bar() { return null; }\n",
            ),
            (
                "src/caller.tsx",
                "import { Foo, Bar } from './target';\nexport function App() { return <Foo />; }\n",
            ),
        ],
        "src/target.tsx",
        "Foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage of Foo from multi-import");
}

#[test]
fn tsjs_t11i_multi_import_no_jsx() {
    // Multi-import, plain function call — no JSX at all
    let (defs, usages) = eval_refs(
        &[
            (
                "src/target.ts",
                "export function foo() {}\nexport function bar() {}\n",
            ),
            (
                "src/caller.ts",
                "import { foo, bar } from './target';\nfoo();\n",
            ),
        ],
        "src/target.ts",
        "foo",
    );
    assert_eq!(defs, 1, "expected 1 definition");
    assert_eq!(usages, 1, "expected 1 usage of foo from multi-import");
}
