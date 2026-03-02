use kdb::fmt::format_workspace;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

// ----------------------------------------------------------------------------------
// kdb/tests/fmt.rs
//
// fn write_file()                                                                L27
// fn write_root_config()                                                         L35
// fn format_workspace_formats_supported_languages_with_readable_rows()           L40
// fn format_workspace_places_block_after_language_preamble()                     L96
// fn format_workspace_is_idempotent()                                           L142
// fn format_workspace_replaces_existing_index_block_in_preamble()               L164
// fn format_workspace_replaces_path_like_header_block_after_file_move()         L182
// fn format_workspace_respects_ignore_patterns_and_skips_unsupported_files()    L201
// fn format_workspace_respects_gitignore_rules()                                L227
// fn format_workspace_generates_markdown_nav_with_outline()                     L248
// fn format_workspace_markdown_nav_is_idempotent()                              L273
// fn format_workspace_markdown_nav_root_file()                                  L297
// fn format_workspace_markdown_nav_no_headings()                                L310
// fn format_workspace_markdown_nav_skips_foreign_frontmatter()                  L325
// fn format_workspace_markdown_nav_force_with_foreign_frontmatter()             L347
// fn assert_index_between()                                                     L366
// ----------------------------------------------------------------------------------

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

#[test]
fn format_workspace_formats_supported_languages_with_readable_rows() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(
        temp.path(),
        "src/lib.rs",
        "pub struct User {}\nimpl User {\n    pub fn name(&self) -> &'static str {\n        \"user\"\n    }\n}\npub fn build() {}\n",
    );
    write_file(
        temp.path(),
        "web/app.ts",
        "export class Service {\n  run() {}\n}\nexport function helper() {}\nconst make = () => 1;\n",
    );
    write_file(
        temp.path(),
        "scripts/tool.py",
        "class Greeter:\n    def hi(self):\n        return \"hi\"\ndef util():\n    return 1\n",
    );
    write_file(
        temp.path(),
        "cmd/main.go",
        "package main\n\ntype Server struct{}\nfunc Start() {}\nfunc (s *Server) Run() {}\n",
    );

    let report = format_workspace(temp.path(), &[], false).expect("format workspace");
    assert_eq!(report.scanned_files, 4);
    assert_eq!(report.updated_files, 4);

    let rust = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read rust file");
    assert!(rust.contains("// src/lib.rs"));
    assert!(rust.contains("// pub struct User"));
    assert!(rust.contains("//   pub fn name()"));
    assert!(rust.contains("// pub fn build()"));

    let ts = fs::read_to_string(temp.path().join("web/app.ts")).expect("read ts file");
    assert!(ts.contains("// web/app.ts"));
    assert!(ts.contains("// export class Service"));
    assert!(ts.contains("//   run()"));
    assert!(ts.contains("// export function helper()"));
    assert!(ts.contains("// const make"));

    let py = fs::read_to_string(temp.path().join("scripts/tool.py")).expect("read python file");
    assert!(py.contains("# scripts/tool.py"));
    assert!(py.contains("# class Greeter"));
    assert!(py.contains("#   def hi()"));
    assert!(py.contains("# def util()"));

    let go = fs::read_to_string(temp.path().join("cmd/main.go")).expect("read go file");
    assert!(go.contains("// cmd/main.go"));
    assert!(go.contains("// type struct Server"));
    assert!(go.contains("// func Start()"));
    assert!(go.contains("//   func Run()"));
}

#[test]
fn format_workspace_places_block_after_language_preamble() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(
        temp.path(),
        "src/lib.rs",
        "#![allow(dead_code)]\n//! crate docs\nuse std::fmt;\n\npub fn run() {}\n",
    );
    write_file(
        temp.path(),
        "web/app.ts",
        "// top comment\nimport fs from \"fs\";\n\nexport function run() {}\n",
    );
    write_file(
        temp.path(),
        "scripts/tool.py",
        "#!/usr/bin/env python3\n\"\"\"tool docs\"\"\"\nimport os\n\ndef run():\n    return os.getcwd()\n",
    );
    write_file(
        temp.path(),
        "cmd/main.go",
        "// top comment\npackage main\n\nimport \"fmt\"\n\nfunc run() {}\n",
    );

    format_workspace(temp.path(), &[], false).expect("format workspace");

    let rust = fs::read_to_string(temp.path().join("src/lib.rs")).expect("read rust file");
    assert_index_between(&rust, "use std::fmt;", "pub fn run() {}", "// src/lib.rs");

    let ts = fs::read_to_string(temp.path().join("web/app.ts")).expect("read ts file");
    assert_index_between(
        &ts,
        "import fs from \"fs\";",
        "export function run() {}",
        "// web/app.ts",
    );

    let py = fs::read_to_string(temp.path().join("scripts/tool.py")).expect("read python file");
    assert_index_between(&py, "import os", "def run():", "# scripts/tool.py");

    let go = fs::read_to_string(temp.path().join("cmd/main.go")).expect("read go file");
    assert_index_between(&go, "import \"fmt\"", "func run() {}", "// cmd/main.go");
}

#[test]
fn format_workspace_is_idempotent() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "main.rs", "fn one() {}\nfn two() {}\n");

    let first = format_workspace(temp.path(), &[], false).expect("first format");
    assert_eq!(first.scanned_files, 1);
    assert_eq!(first.updated_files, 1);

    let once = fs::read_to_string(temp.path().join("main.rs")).expect("read once");
    assert!(once.contains("// fn one()"));
    assert!(once.contains("// fn two()"));

    let second = format_workspace(temp.path(), &[], false).expect("second format");
    assert_eq!(second.scanned_files, 1);
    assert_eq!(second.updated_files, 0);

    let twice = fs::read_to_string(temp.path().join("main.rs")).expect("read twice");
    assert_eq!(once, twice);
}

#[test]
fn format_workspace_replaces_existing_index_block_in_preamble() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "lib.rs",
        "use std::fmt;\n\n// ## Index\n//\n// fn stale()  L1\n\nfn fresh() {}\n",
    );

    let report = format_workspace(temp.path(), &[], false).expect("format workspace");
    assert_eq!(report.updated_files, 1);

    let rust = fs::read_to_string(temp.path().join("lib.rs")).expect("read rust file");
    assert!(!rust.contains("stale"));
    assert!(rust.contains("// fn fresh()"));
}

#[test]
fn format_workspace_replaces_path_like_header_block_after_file_move() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/new_name.rs",
        "// old/path.rs\n//\n// fn stale()  L1\n\nfn fresh() {}\n",
    );

    let report = format_workspace(temp.path(), &[], false).expect("format workspace");
    assert_eq!(report.updated_files, 1);

    let rust = fs::read_to_string(temp.path().join("src/new_name.rs")).expect("read rust file");
    assert!(rust.contains("// src/new_name.rs"));
    assert!(!rust.contains("old/path.rs"));
    assert!(!rust.contains("stale"));
}

#[test]
fn format_workspace_respects_ignore_patterns_and_skips_unsupported_files() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(temp.path(), "src/main.rs", "fn live() {}\n");
    write_file(temp.path(), "notes.txt", "plain text\n");
    write_file(
        temp.path(),
        "vendor/tool.py",
        "def hidden():\n    return 1\n",
    );

    let report =
        format_workspace(temp.path(), &["vendor/**".to_string()], false).expect("format workspace");
    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.updated_files, 1);

    let txt = fs::read_to_string(temp.path().join("notes.txt")).expect("read txt file");
    assert_eq!(txt, "plain text\n");

    let ignored =
        fs::read_to_string(temp.path().join("vendor/tool.py")).expect("read ignored file");
    assert!(!ignored.contains("## Index"));
}

#[test]
fn format_workspace_respects_gitignore_rules() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), ".gitignore", "vendor/\n");

    write_file(temp.path(), "src/main.rs", "fn live() {}\n");
    write_file(temp.path(), "vendor/hidden.rs", "fn hidden() {}\n");

    let report = format_workspace(temp.path(), &[], false).expect("format workspace");
    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.updated_files, 1);

    let live = fs::read_to_string(temp.path().join("src/main.rs")).expect("read live file");
    assert!(live.contains("// src/main.rs"));

    let hidden =
        fs::read_to_string(temp.path().join("vendor/hidden.rs")).expect("read hidden file");
    assert!(!hidden.contains("// vendor/hidden.rs"));
}

#[test]
fn format_workspace_generates_markdown_nav_with_outline() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(
        temp.path(),
        "docs/arch/overview.md",
        "# Architecture\n\n## Storage\n\n## Query Engine\n",
    );

    let report = format_workspace(temp.path(), &[], false).expect("format workspace");
    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.updated_files, 1);

    let md = fs::read_to_string(temp.path().join("docs/arch/overview.md")).expect("read md file");
    assert!(md.contains("path: docs/arch/overview.md"));
    assert!(md.contains("outline: |"));
    assert!(md.contains("• Architecture"));
    assert!(md.contains("◦ Storage"));
    assert!(md.contains("◦ Query Engine"));
    // Must be wrapped in frontmatter delimiters.
    assert!(md.starts_with("---\n"));
}

#[test]
fn format_workspace_markdown_nav_is_idempotent() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(
        temp.path(),
        "notes/index.md",
        "# Notes\n\n## First\n\n## Second\n",
    );

    let first = format_workspace(temp.path(), &[], false).expect("first format");
    assert_eq!(first.updated_files, 1);

    let once = fs::read_to_string(temp.path().join("notes/index.md")).expect("read once");
    assert!(once.contains("path: notes/index.md"));

    let second = format_workspace(temp.path(), &[], false).expect("second format");
    assert_eq!(second.updated_files, 0);

    let twice = fs::read_to_string(temp.path().join("notes/index.md")).expect("read twice");
    assert_eq!(once, twice);
}

#[test]
fn format_workspace_markdown_nav_root_file() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(temp.path(), "readme.md", "# Hello\n\nWorld.\n");

    format_workspace(temp.path(), &[], false).expect("format workspace");

    let md = fs::read_to_string(temp.path().join("readme.md")).expect("read md file");
    assert!(md.contains("path: readme.md"));
}

#[test]
fn format_workspace_markdown_nav_no_headings() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(temp.path(), "empty.md", "Just text.\n");

    format_workspace(temp.path(), &[], false).expect("format workspace");

    let md = fs::read_to_string(temp.path().join("empty.md")).expect("read md file");
    assert!(md.contains("path: empty.md"));
    // No outline key when there are no headings.
    assert!(!md.contains("outline:"));
}

#[test]
fn format_workspace_markdown_nav_skips_foreign_frontmatter() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(
        temp.path(),
        "doc.md",
        "---\ntitle: Test\n---\n\n# Heading\n",
    );

    let report = format_workspace(temp.path(), &[], false).expect("format workspace");
    // Should skip the file and emit a warning.
    assert_eq!(report.updated_files, 0);
    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].message.contains("existing frontmatter"));

    let md = fs::read_to_string(temp.path().join("doc.md")).expect("read md file");
    // File should be unchanged.
    assert_eq!(md, "---\ntitle: Test\n---\n\n# Heading\n");
}

#[test]
fn format_workspace_markdown_nav_force_with_foreign_frontmatter() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    write_file(
        temp.path(),
        "doc.md",
        "---\ntitle: Test\n---\n\n# Heading\n",
    );

    let report = format_workspace(temp.path(), &[], true).expect("format workspace");
    assert_eq!(report.updated_files, 1);

    let md = fs::read_to_string(temp.path().join("doc.md")).expect("read md file");
    assert!(md.contains("title: Test"));
    assert!(md.contains("path: doc.md"));
    assert!(md.contains("• Heading"));
}

fn assert_index_between(content: &str, before: &str, after: &str, header: &str) {
    let before_pos = content.find(before).expect("before marker missing");
    let header_pos = content.find(header).expect("index header missing");
    let after_pos = content.find(after).expect("after marker missing");

    assert!(
        header_pos > before_pos,
        "index header should follow preamble"
    );
    assert!(
        header_pos < after_pos,
        "index header should precede declarations"
    );
}
