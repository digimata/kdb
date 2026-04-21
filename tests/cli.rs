use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

// ---------------------------------------------------------------------------------
// projects/kdb/tests/cli.rs
//
// fn write_file()                                                               L87
// fn bin()                                                                      L95
// fn write_root_config()                                                        L99
// fn write_symbol_refs_rust_fixture()                                          L103
// fn check_exits_zero_for_clean_vault()                                        L147
// fn check_exits_one_for_broken_links()                                        L165
// fn check_respects_index_ignore_patterns_from_config()                        L183
// fn check_respects_gitignore_rules()                                          L210
// fn check_orphan_only_shows_orphan_count_hint_without_listing()               L234
// fn check_orphans_flag_lists_orphan_files()                                   L256
// fn check_scopes_output_to_explicit_subtree_path()                            L278
// fn check_scoped_subtree_still_validates_cross_subtree_links()                L303
// fn check_errors_when_root_marker_missing()                                   L331
// fn fmt_generates_code_index_headers_for_supported_files()                    L347
// fn fmt_warns_when_nonstandard_index_rows_are_removed()                       L369
// fn fmt_scopes_to_explicit_file_path()                                        L392
// fn fmt_respects_gitignore_rules()                                            L416
// fn fmt_explicit_path_can_format_gitignored_file()                            L438
// fn tree_prints_filtered_directory_structure()                                L459
// fn tree_level_option_matches_tree_l_flag()                                   L493
// fn tree_json_dirs_only_and_all_flags_are_supported()                         L515
// fn tree_full_path_flag_prints_full_relative_paths()                          L541
// fn tree_ignore_pattern_flag_excludes_matches()                               L559
// fn tree_pattern_flag_includes_only_matching_subtrees()                       L580
// fn symbols_prints_markdown_heading_symbols()                                 L604
// fn symbols_supports_public_filter_for_code_files()                           L630
// fn symbols_json_outputs_structured_rows()                                    L662
// fn symbols_selector_prints_multiple_matching_bodies()                        L690
// fn symbols_selector_shows_line_number_gutter()                               L734
// fn symbols_selector_accepts_multiple_selectors()                             L767
// fn symbols_selector_includes_doc_comments()                                  L802
// fn symbols_selector_includes_attributes_with_docs()                          L837
// fn symbols_selector_supports_qualified_names()                               L866
// fn symbols_selector_json_outputs_body_and_metadata()                         L913
// fn symbols_selector_respects_public_filter()                                 L958
// fn symbols_selector_extracts_markdown_sections_by_slug()                     L999
// fn symbols_selector_markdown_json_outputs_body_and_metadata()               L1033
// fn symbols_selector_markdown_slug_not_found_errors()                        L1078
// fn refs_lists_inbound_references_for_file_target()                          L1102
// fn refs_lists_inbound_references_for_heading_target()                       L1129
// fn refs_count_prints_number_of_inbound_references()                         L1160
// fn refs_json_outputs_structured_rows()                                      L1181
// fn refs_symbol_lists_inbound_references_for_code_symbol()                   L1208
// fn refs_symbol_count_prints_number_of_references()                          L1232
// fn refs_symbol_json_outputs_structured_rows()                               L1252
// fn refs_symbol_context_shows_surrounding_lines_in_text_output()             L1282
// fn refs_symbol_context_zero_keeps_single_line_output()                      L1307
// fn refs_markdown_context_flag_errors_without_symbol()                       L1330
// fn refs_symbol_json_ignores_context_flag()                                  L1351
// fn refs_symbol_count_ignores_context_flag()                                 L1378
// fn deps_lists_outbound_dependencies_for_file_target()                       L1400
// fn deps_json_outputs_structured_rows()                                      L1433
// fn deps_supports_rust_code_file_targets()                                   L1470
// fn deps_supports_rust_nested_crate_roots_in_monorepos()                     L1497
// fn deps_supports_rust_workspace_sibling_crate_imports()                     L1538
// fn deps_supports_rust_workspace_renamed_path_dependencies()                 L1584
// fn deps_supports_rust_custom_lib_path_mod_declarations()                    L1630
// fn deps_supports_rust_custom_bin_path_mod_declarations()                    L1654
// fn deps_supports_rust_custom_lib_path_cross_crate_fallback()                L1678
// fn deps_supports_typescript_code_file_targets()                             L1719
// fn deps_supports_typescript_tsconfig_path_aliases()                         L1747
// fn deps_supports_typescript_monorepo_per_package_tsconfig_path_aliases()    L1775
// fn refs_resolves_typescript_monorepo_per_package_tsconfig_path_aliases()    L1820
// fn deps_supports_typescript_workspace_package_imports()                     L1866
// fn deps_supports_python_code_file_targets()                                 L1914
// fn deps_supports_python_pyproject_src_layout_package_imports()              L1950
// fn deps_supports_python_setup_py_src_layout_fallback()                      L1988
// fn deps_supports_python_poetry_src_namespace_packages()                     L2013
// fn deps_python_canonicalizes_resolved_file_case_on_case_insensitive_fs()    L2048
// fn deps_supports_go_code_file_targets()                                     L2090
// fn deps_supports_go_workspace_use_cross_module_imports()                    L2119
// fn deps_supports_go_workspace_replace_local_path_overrides()                L2149
// fn graph_is_stubbed_with_clear_message()                                    L2188
// fn init_creates_kdb_directory_and_default_config()                          L2201
// fn init_errors_if_kdb_directory_already_exists()                            L2227
// ---------------------------------------------------------------------------------

fn write_file(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write fixture file");
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kdb")
}

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .current_dir(root)
        .args(args)
        .output()
        .expect("run kdb command")
}

fn write_root_config(root: &Path) {
    write_file(
        root,
        ".kdb/config.toml",
        "[workspace]\nname = \"fixture\"\n",
    );
}

fn write_symbol_refs_rust_fixture(root: &Path) {
    write_root_config(root);
    write_file(
        root,
        "src/lib.rs",
        "pub mod cmd;\npub mod lsp;\npub mod project;\n",
    );
    write_file(root, "src/project/mod.rs", "pub mod root;\n");
    write_file(
        root,
        "src/project/root.rs",
        concat!(
            "use std::path::{Path, PathBuf};\n\n",
            "pub fn find_root(start: &Path) -> Option<PathBuf> {\n",
            "    Some(start.to_path_buf())\n",
            "}\n",
        ),
    );
    write_file(root, "src/lsp/mod.rs", "pub mod backend;\n");
    write_file(
        root,
        "src/cmd.rs",
        concat!(
            "use std::path::Path;\n",
            "use crate::project::root::find_root;\n\n",
            "pub fn run() {\n",
            "    let _ = find_root(Path::new(\".\"));\n",
            "}\n",
        ),
    );
    write_file(
        root,
        "src/lsp/backend.rs",
        concat!(
            "use std::path::Path;\n",
            "use crate::project::root::find_root;\n\n",
            "pub fn refresh() {\n",
            "    let _ = find_root(Path::new(\"/tmp\"));\n",
            "}\n",
        ),
    );
}

#[test]
fn check_exits_zero_for_clean_vault() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[B](b.md#target)\n");
    write_file(temp.path(), "b.md", "# B\n\n## Target\n\n[A](a.md#a)\n");

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb check: no issues found"));
}

#[test]
fn check_exits_one_for_broken_links() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[Missing](missing.md)\n");

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("broken link"));
    assert!(stdout.contains("missing.md"));
}

#[test]
fn check_respects_index_ignore_patterns_from_config() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[workspace]\nname = \"fixture\"\n[index]\nignore = [\"archive/**\"]\n",
    );
    write_file(temp.path(), "a.md", "# A\n\n[B](b.md)\n");
    write_file(temp.path(), "b.md", "# B\n\n[A](a.md)\n");
    write_file(
        temp.path(),
        "archive/bad.md",
        "# Bad\n\n[Missing](missing.md)\n",
    );

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb check: no issues found"));
}

#[test]
fn check_respects_gitignore_rules() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), ".gitignore", "archive/\n");
    write_file(temp.path(), "a.md", "# A\n\n[B](b.md)\n");
    write_file(temp.path(), "b.md", "# B\n\n[A](a.md)\n");
    write_file(
        temp.path(),
        "archive/bad.md",
        "# Bad\n\n[Missing](missing.md)\n",
    );

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb check: no issues found"));
}

#[test]
fn check_orphan_only_shows_orphan_count_hint_without_listing() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\nNo links here.\n");
    write_file(temp.path(), "b.md", "# B\n\nNo links here either.\n");

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 orphan files (run `kdb check --orphans` to list)"));
    assert!(stdout.contains("2 warnings"));
    assert!(!stdout.contains("a.md orphan file (0 inbound links)"));
    assert!(!stdout.contains("b.md orphan file (0 inbound links)"));
    assert!(!stdout.contains("broken link"));
}

#[test]
fn check_orphans_flag_lists_orphan_files() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\nNo links here.\n");
    write_file(temp.path(), "b.md", "# B\n\nNo links here either.\n");

    let output = Command::new(bin())
        .arg("check")
        .arg("--orphans")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a.md orphan file (0 inbound links)"));
    assert!(stdout.contains("b.md orphan file (0 inbound links)"));
    assert!(stdout.contains("2 warnings"));
    assert!(!stdout.contains("run `kdb check --orphans` to list"));
}

#[test]
fn check_scopes_output_to_explicit_subtree_path() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "crates/agent/guide.md", "# Guide\n");
    write_file(
        temp.path(),
        "docs/broken.md",
        "# Broken\n\n[Missing](missing.md)\n",
    );

    let output = Command::new(bin())
        .arg("check")
        .arg("--orphans")
        .arg(temp.path().join("crates/agent"))
        .output()
        .expect("run scoped kdb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("crates/agent/guide.md orphan file (0 inbound links)"));
    assert!(!stdout.contains("docs/broken.md"));
    assert!(!stdout.contains("missing.md"));
}

#[test]
fn check_scoped_subtree_still_validates_cross_subtree_links() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "crates/agent/guide.md",
        "# Guide\n\n[Missing Docs](../../docs/missing.md)\n",
    );
    write_file(
        temp.path(),
        "docs/unrelated.md",
        "# Unrelated\n\n[Missing](missing.md)\n",
    );

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path().join("crates/agent"))
        .output()
        .expect("run scoped kdb check with cross-subtree link");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("crates/agent/guide.md"));
    assert!(stdout.contains("docs/missing.md"));
    assert!(!stdout.contains("docs/unrelated.md"));
}

#[test]
fn check_errors_when_root_marker_missing() {
    let temp = tempdir().expect("tempdir");
    write_file(temp.path(), "a.md", "# A\n");

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("could not find .kdb"));
}

#[test]
fn fmt_generates_code_index_headers_for_supported_files() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "src/main.rs", "fn run() {}\n");

    let output = Command::new(bin())
        .arg("fmt")
        .arg(temp.path())
        .output()
        .expect("run kdb fmt");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb fmt: updated 1 of 1 files"));

    let formatted =
        fs::read_to_string(temp.path().join("src/main.rs")).expect("read formatted rust file");
    assert!(formatted.contains("// src/main.rs"));
    assert!(formatted.contains("// fn run()"));
}

#[test]
fn fmt_warns_when_nonstandard_index_rows_are_removed() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/main.rs",
        "// src/main.rs\n//\n// totally custom row\nfn run() {}\n",
    );

    let output = Command::new(bin())
        .arg("fmt")
        .arg(temp.path())
        .output()
        .expect("run kdb fmt");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("kdb fmt: 1 warning(s)"));
    assert!(stderr.contains("removed 1 non-standard index row"));
    assert!(stderr.contains("src/main.rs"));
}

#[test]
fn fmt_scopes_to_explicit_file_path() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "src/one.rs", "fn one() {}\n");
    write_file(temp.path(), "src/two.rs", "fn two() {}\n");

    let output = Command::new(bin())
        .arg("fmt")
        .arg(temp.path().join("src/one.rs"))
        .output()
        .expect("run kdb fmt src/one.rs");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb fmt: updated 1 of 1 files"));

    let one = fs::read_to_string(temp.path().join("src/one.rs")).expect("read one.rs");
    let two = fs::read_to_string(temp.path().join("src/two.rs")).expect("read two.rs");
    assert!(one.contains("// src/one.rs"));
    assert!(one.contains("// fn one()"));
    assert!(!two.contains("## Index"));
}

#[test]
fn fmt_respects_gitignore_rules() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), ".gitignore", "vendor/\n");
    write_file(temp.path(), "src/main.rs", "fn run() {}\n");
    write_file(temp.path(), "vendor/hidden.rs", "fn hidden() {}\n");

    let output = Command::new(bin())
        .arg("fmt")
        .arg(temp.path())
        .output()
        .expect("run kdb fmt");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb fmt: updated 1 of 1 files"));

    let hidden = fs::read_to_string(temp.path().join("vendor/hidden.rs")).expect("read hidden.rs");
    assert!(!hidden.contains("// vendor/hidden.rs"));
}

#[test]
fn fmt_explicit_path_can_format_gitignored_file() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), ".gitignore", "src/ignored.rs\n");
    write_file(temp.path(), "src/ignored.rs", "fn ignored() {}\n");

    let output = Command::new(bin())
        .arg("fmt")
        .arg(temp.path().join("src/ignored.rs"))
        .output()
        .expect("run kdb fmt src/ignored.rs");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("kdb fmt: updated 1 of 1 files"));

    let ignored = fs::read_to_string(temp.path().join("src/ignored.rs")).expect("read ignored.rs");
    assert!(ignored.contains("// src/ignored.rs"));
}

#[test]
fn tree_prints_filtered_directory_structure() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[workspace]\nname = \"fixture\"\n[index]\nignore = [\"archive/**\"]\n",
    );
    write_file(temp.path(), "src/main.rs", "fn main() {}\n");
    write_file(temp.path(), "notes/todo.md", "# TODO\n");
    write_file(temp.path(), ".hidden.md", "# hidden\n");
    write_file(temp.path(), "archive/old.md", "# old\n");
    write_file(temp.path(), "target/generated.txt", "generated\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("tree")
        .output()
        .expect("run kdb tree");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_line = stdout.lines().next().expect("non-empty output");
    assert!(
        std::path::Path::new(first_line).is_absolute(),
        "tree root should be an absolute path, got: {first_line}"
    );
    assert!(stdout.contains("notes"));
    assert!(stdout.contains("src"));
    assert!(!stdout.contains(".hidden.md"));
    assert!(!stdout.contains("archive"));
    assert!(!stdout.contains("target"));
}

#[test]
fn tree_level_option_matches_tree_l_flag() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a/b/c/deep.md", "# deep\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("tree")
        .arg("-L")
        .arg("1")
        .output()
        .expect("run kdb tree -L 1");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip the root line (absolute path) — only check tree body for children.
    let body: String = stdout.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert!(body.contains("a"));
    assert!(!body.contains("b"));
}

#[test]
fn tree_json_dirs_only_and_all_flags_are_supported() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/page.md", "# page\n");
    write_file(temp.path(), ".private/notes.md", "# private\n");
    write_file(temp.path(), "root.md", "# root\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("tree")
        .arg("-J")
        .arg("-d")
        .arg("-a")
        .output()
        .expect("run kdb tree -J -d -a");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse tree json");
    assert_eq!(json["kind"], "directory");
    let children = json["children"].as_array().expect("children array");
    assert!(children.iter().any(|node| node["name"] == ".private"));
    assert!(children.iter().any(|node| node["name"] == "docs"));
    assert!(!children.iter().any(|node| node["name"] == "root.md"));
}

#[test]
fn tree_full_path_flag_prints_full_relative_paths() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "src/main.rs", "fn main() {}\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("tree")
        .arg("-f")
        .output()
        .expect("run kdb tree -f");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("src/main.rs"));
}

#[test]
fn tree_ignore_pattern_flag_excludes_matches() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/page.md", "# page\n");
    write_file(temp.path(), "src/main.rs", "fn main() {}\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("tree")
        .arg("-I")
        .arg("docs/**")
        .output()
        .expect("run kdb tree -I docs/**");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("docs"));
    assert!(stdout.contains("src"));
}

#[test]
fn tree_pattern_flag_includes_only_matching_subtrees() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/page.md", "# page\n");
    write_file(temp.path(), "src/main.rs", "fn main() {}\n");
    write_file(temp.path(), "tests/cli.rs", "// test\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("tree")
        .arg("-P")
        .arg("src/**")
        .output()
        .expect("run kdb tree -P src/**");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("src"));
    assert!(stdout.contains("main.rs"));
    assert!(!stdout.contains("docs"));
    assert!(!stdout.contains("tests"));
}

#[test]
fn symbols_prints_markdown_heading_symbols() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/page.md",
        "# Top\n\n## Child\n\n### Leaf\n",
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("docs/page.md"))
        .output()
        .expect("run kdb symbols");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Top"));
    assert!(stdout.contains("## Child"));
    assert!(stdout.contains("### Leaf"));
    assert!(stdout.contains("L1"));
    assert!(stdout.contains("L3"));
    assert!(stdout.contains("L5"));
}

#[test]
fn symbols_supports_public_filter_for_code_files() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        "pub fn open() {}\nfn hidden() {}\n",
    );

    let all_output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .output()
        .expect("run kdb symbols");
    assert!(all_output.status.success());
    let all_stdout = String::from_utf8_lossy(&all_output.stdout);
    assert!(all_stdout.contains("fn open()"));
    assert!(all_stdout.contains("fn hidden()"));

    let public_output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("--public")
        .output()
        .expect("run kdb symbols --public");
    assert!(public_output.status.success());
    let public_stdout = String::from_utf8_lossy(&public_output.stdout);
    assert!(public_stdout.contains("fn open()"));
    assert!(!public_stdout.contains("fn hidden()"));
}

#[test]
fn symbols_json_outputs_structured_rows() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        "pub fn open() {}\nfn hidden() {}\n",
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("--json")
        .arg("--public")
        .output()
        .expect("run kdb symbols --json --public");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse symbols json");
    let rows = json.as_array().expect("symbols json array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "fn");
    assert_eq!(rows[0]["display_kind"], "pub fn");
    assert_eq!(rows[0]["name"], "open");
    assert_eq!(rows[0]["public"], true);
}

#[test]
fn symbols_selector_prints_multiple_matching_bodies() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "pub struct Backend;\n\n",
            "impl Backend {\n",
            "    pub fn open(&self) -> i32 {\n",
            "        1\n",
            "    }\n\n",
            "    fn hidden(&self) -> i32 {\n",
            "        0\n",
            "    }\n",
            "}\n\n",
            "pub fn open() -> i32 {\n",
            "    2\n",
            "}\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("open")
        .output()
        .expect("run kdb symbols -s open");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Both matches appear with line gutter
    assert!(stdout.contains("| pub fn open(&self) -> i32"));
    assert!(stdout.contains("| pub fn open() -> i32"));
    // Separate symbols are separated by a blank line
    let parts: Vec<&str> = stdout.split("\n\n").collect();
    assert!(
        parts.len() >= 2,
        "expected blank line between symbol bodies"
    );
}

#[test]
fn symbols_selector_shows_line_number_gutter() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "pub struct Backend;\n\n",
            "impl Backend {\n",
            "    pub fn open(&self) -> i32 {\n",
            "        1\n",
            "    }\n",
            "}\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("Backend::open")
        .output()
        .expect("run kdb symbols -s");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Line numbers reflect actual file positions (open starts at line 4)
    assert!(stdout.contains("4 | pub fn open(&self) -> i32 {"));
    assert!(stdout.contains("5 |         1"));
    assert!(stdout.contains("6 |     }"));
}

#[test]
fn symbols_selector_accepts_multiple_selectors() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "pub struct Backend;\n\n",
            "impl Backend {\n",
            "    pub fn open(&self) -> i32 {\n",
            "        1\n",
            "    }\n",
            "}\n\n",
            "pub fn close() -> i32 {\n",
            "    0\n",
            "}\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("Backend::open")
        .arg("close")
        .output()
        .expect("run kdb symbols -s with multiple selectors");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("| pub fn open(&self) -> i32"));
    assert!(stdout.contains("| pub fn close() -> i32"));
}

#[test]
fn symbols_selector_includes_doc_comments() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "/// Create a new backend instance.\n",
            "///\n",
            "/// Returns the default backend.\n",
            "pub fn create() -> i32 {\n",
            "    1\n",
            "}\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("create")
        .output()
        .expect("run kdb symbols -s create");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Doc comments are included before the function body
    assert!(stdout.contains("| /// Create a new backend instance."));
    assert!(stdout.contains("| /// Returns the default backend."));
    assert!(stdout.contains("| pub fn create() -> i32 {"));
    // Start line is the first doc comment line, not the fn line
    assert!(stdout.starts_with("1 | ///"));
}

#[test]
fn symbols_selector_includes_attributes_with_docs() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "/// A backend type.\n",
            "#[derive(Debug)]\n",
            "pub struct Backend;\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("Backend")
        .output()
        .expect("run kdb symbols -s Backend");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("| /// A backend type."));
    assert!(stdout.contains("| #[derive(Debug)]"));
    assert!(stdout.contains("| pub struct Backend;"));
}

#[test]
fn symbols_selector_supports_qualified_names() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "pub struct Backend;\n\n",
            "impl Backend {\n",
            "    pub fn open(&self) -> i32 {\n",
            "        1\n",
            "    }\n",
            "}\n\n",
            "pub fn open() -> i32 {\n",
            "    2\n",
            "}\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("Backend::open")
        .output()
        .expect("run kdb symbols -s Backend::open");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pub fn open(&self) -> i32"));
    assert!(!stdout.contains("pub fn open() -> i32"));

    // Dot syntax works the same as :: syntax
    let dot_output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("Backend.open")
        .output()
        .expect("run kdb symbols -s Backend.open");

    assert!(dot_output.status.success());
    let dot_stdout = String::from_utf8_lossy(&dot_output.stdout);
    assert_eq!(stdout, dot_stdout);
}

#[test]
fn symbols_selector_json_outputs_body_and_metadata() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "pub struct Backend;\n\n",
            "impl Backend {\n",
            "    pub fn open(&self) -> i32 {\n",
            "        1\n",
            "    }\n",
            "}\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("Backend::open")
        .arg("--json")
        .output()
        .expect("run kdb symbols -s Backend::open --json");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse symbols body json");
    let rows = json.as_array().expect("symbols body json array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["file"], "src/lib.rs");
    assert_eq!(rows[0]["kind"], "fn");
    assert_eq!(rows[0]["display_kind"], "pub fn");
    assert_eq!(rows[0]["name"], "open");
    assert_eq!(rows[0]["parent"], "Backend");
    assert_eq!(rows[0]["public"], true);
    assert!(rows[0]["line"].as_u64().is_some());
    assert!(rows[0]["end_line"].as_u64().is_some());
    assert!(
        rows[0]["body"]
            .as_str()
            .is_some_and(|body| body.contains("pub fn open(&self)"))
    );
}

#[test]
fn symbols_selector_respects_public_filter() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        concat!(
            "pub struct Backend;\n\n",
            "impl Backend {\n",
            "    fn hidden(&self) -> i32 {\n",
            "        0\n",
            "    }\n",
            "}\n",
        ),
    );

    let visible = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("hidden")
        .output()
        .expect("run kdb symbols -s hidden");
    assert!(visible.status.success());
    assert!(String::from_utf8_lossy(&visible.stdout).contains("fn hidden(&self)"));

    let filtered = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("src/lib.rs"))
        .arg("-s")
        .arg("hidden")
        .arg("--public")
        .output()
        .expect("run kdb symbols -s hidden --public");
    assert_eq!(filtered.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&filtered.stderr);
    assert!(stderr.contains("symbol not found"));
    assert!(stderr.contains("--public filter"));
}

#[test]
fn symbols_selector_extracts_markdown_sections_by_slug() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/page.md",
        concat!(
            "# Top\n\n",
            "## SOP-3 Refactor Cleanup\n\n",
            "Cleanup details.\n\n",
            "### Nested Step\n\n",
            "- remove dead code\n\n",
            "## SOP-4 Bugfix\n\n",
            "Bugfix details.\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("docs/page.md"))
        .arg("-s")
        .arg("#SOP-3-REFACTOR-CLEANUP")
        .output()
        .expect("run kdb symbols markdown -s");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## SOP-3 Refactor Cleanup"));
    assert!(stdout.contains("### Nested Step"));
    assert!(stdout.contains("- remove dead code"));
    assert!(!stdout.contains("## SOP-4 Bugfix"));
}

#[test]
fn symbols_selector_markdown_json_outputs_body_and_metadata() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/page.md",
        concat!(
            "# Top\n\n",
            "## SOP-3 Refactor Cleanup\n\n",
            "Cleanup details.\n\n",
            "### Nested Step\n\n",
            "- remove dead code\n\n",
            "## SOP-4 Bugfix\n\n",
            "Bugfix details.\n",
        ),
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("docs/page.md"))
        .arg("-s")
        .arg("sop-3-refactor-cleanup")
        .arg("--json")
        .output()
        .expect("run kdb symbols markdown -s --json");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse symbols body json");
    let rows = json.as_array().expect("symbols body json array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["file"], "docs/page.md");
    assert_eq!(rows[0]["kind"], "heading");
    assert_eq!(rows[0]["display_kind"], "##");
    assert_eq!(rows[0]["name"], "SOP-3 Refactor Cleanup");
    assert_eq!(rows[0]["public"], true);
    assert_eq!(rows[0]["line"], 3);
    assert_eq!(rows[0]["end_line"], 10);
    assert!(
        rows[0]["body"].as_str().is_some_and(
            |body| body.contains("### Nested Step") && !body.contains("## SOP-4 Bugfix")
        )
    );
}

#[test]
fn symbols_selector_markdown_slug_not_found_errors() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/page.md",
        "# Top\n\n## SOP-3 Refactor Cleanup\n\nCleanup details.\n",
    );

    let output = Command::new(bin())
        .arg("symbols")
        .arg(temp.path().join("docs/page.md"))
        .arg("-s")
        .arg("SOP-3 Refactor Cleanup")
        .output()
        .expect("run kdb symbols markdown slug not found");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("symbol not found"));
    assert!(stderr.contains("SOP-3 Refactor Cleanup"));
}

#[test]
fn refs_lists_inbound_references_for_file_target() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n\n## useEffect\n");
    write_file(
        temp.path(),
        "tutorial.md",
        "# Tutorial\n\n[React Hooks](docs/hooks.md)\n",
    );
    write_file(temp.path(), "index.md", "# Index\n\n[[docs/hooks]]\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("docs/hooks.md")
        .output()
        .expect("run kdb refs");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tutorial.md:"));
    assert!(stdout.contains("docs/hooks.md"));
    assert!(stdout.contains("index.md:"));
    assert!(stdout.contains("[[docs/hooks]]"));
}

#[test]
fn refs_lists_inbound_references_for_heading_target() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n\n## useEffect\n");
    write_file(
        temp.path(),
        "components.md",
        "# Components\n\n[useEffect](docs/hooks.md#useEffect)\n",
    );
    write_file(
        temp.path(),
        "patterns.md",
        "# Patterns\n\n[[docs/hooks#useEffect]]\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("docs/hooks.md#useEffect")
        .output()
        .expect("run kdb refs heading");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("components.md:"));
    assert!(stdout.contains("docs/hooks.md#useEffect"));
    assert!(stdout.contains("patterns.md:"));
    assert!(stdout.contains("[[docs/hooks#useEffect]]"));
}

#[test]
fn refs_count_prints_number_of_inbound_references() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n");
    write_file(temp.path(), "a.md", "# A\n\n[Hooks](docs/hooks.md)\n");
    write_file(temp.path(), "b.md", "# B\n\n[[docs/hooks]]\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("docs/hooks.md")
        .arg("--count")
        .output()
        .expect("run kdb refs --count");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn refs_json_outputs_structured_rows() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n");
    write_file(temp.path(), "a.md", "# A\n\n[Hooks](docs/hooks.md)\n");
    write_file(temp.path(), "b.md", "# B\n\n[[docs/hooks]]\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("docs/hooks.md")
        .arg("--json")
        .output()
        .expect("run kdb refs --json");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse refs json");
    let rows = json.as_array().expect("refs json array");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row["source_file"] == "a.md"));
    assert!(rows.iter().any(|row| row["source_file"] == "b.md"));
    assert!(rows.iter().all(|row| row["line"].is_number()));
    assert!(rows.iter().all(|row| row["column"].is_number()));
    assert!(rows.iter().all(|row| row["raw"].is_string()));
}

#[test]
fn refs_symbol_lists_inbound_references_for_code_symbol() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .output()
        .expect("run kdb refs -s");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("── src/project/root.rs"));
    assert!(stdout.contains("pub fn find_root(start: &Path)"));
    assert!(stdout.contains("── src/cmd.rs"));
    assert!(stdout.contains("find_root(Path::new(\".\"))"));
    assert!(stdout.contains("── src/lsp/backend.rs"));
    assert!(stdout.contains("find_root(Path::new(\"/tmp\"))"));
}

#[test]
fn refs_symbol_count_prints_number_of_references() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .arg("--count")
        .output()
        .expect("run kdb refs -s --count");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn refs_symbol_json_outputs_structured_rows() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .arg("--json")
        .output()
        .expect("run kdb refs -s --json");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse refs -s json");
    let rows = json.as_array().expect("refs -s json array");
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().any(|row| {
        row["source_file"] == "src/project/root.rs" && row["is_definition"] == true
    }));
    assert!(
        rows.iter()
            .filter(|row| row["is_definition"] == false)
            .all(|row| row["line"].is_number() && row["column"].is_number())
    );
    assert!(rows.iter().all(|row| row["snippet"].is_string()));
}

#[test]
fn refs_symbol_context_shows_surrounding_lines_in_text_output() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .arg("-c")
        .arg("1")
        .output()
        .expect("run kdb refs -s -c 1");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("src/cmd.rs:5:"));
    assert!(stdout.contains("  4 | pub fn run() {"));
    assert!(stdout.contains("> 5 |     let _ = find_root(Path::new(\".\"));"));
    assert!(stdout.contains("  6 | }"));
    assert!(stdout.contains("--"));
}

#[test]
fn refs_symbol_context_zero_keeps_single_line_output() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .arg("-c")
        .arg("0")
        .output()
        .expect("run kdb refs -s -c 0");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("── src/project/root.rs"));
    assert!(stdout.contains("── src/cmd.rs"));
    assert!(!stdout.contains(" | "));
}

#[test]
fn refs_markdown_context_flag_errors_without_symbol() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n");
    write_file(temp.path(), "a.md", "# A\n\n[Hooks](docs/hooks.md)\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("docs/hooks.md")
        .arg("-c")
        .arg("1")
        .output()
        .expect("run kdb refs markdown -c 1");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--context is currently supported only with --symbol"));
}

#[test]
fn refs_symbol_json_ignores_context_flag() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .arg("--json")
        .arg("-c")
        .arg("2")
        .output()
        .expect("run kdb refs -s --json -c 2");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse refs -s json");
    let rows = json.as_array().expect("refs -s json array");
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| {
        row.as_object()
            .is_some_and(|obj| !obj.contains_key("context_lines"))
    }));
}

#[test]
fn refs_symbol_count_ignores_context_flag() {
    let temp = tempdir().expect("tempdir");
    write_symbol_refs_rust_fixture(temp.path());

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("src/project/root.rs")
        .arg("-s")
        .arg("find_root")
        .arg("--count")
        .arg("-c")
        .arg("2")
        .output()
        .expect("run kdb refs -s --count -c 2");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn deps_lists_outbound_dependencies_for_file_target() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/tutorial.md",
        "# Tutorial\n\n[State](state.md)\n[Hooks](hooks.md)\n[[components#Props]]\n[Hooks Again](./hooks.md)\n",
    );
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n");
    write_file(
        temp.path(),
        "docs/components.md",
        "# Components\n\n## Props\n",
    );
    write_file(temp.path(), "docs/state.md", "# State\n");

    let output = Command::new(bin())
        .arg("deps")
        .current_dir(temp.path())
        .arg("docs/tutorial.md")
        .output()
        .expect("run kdb deps");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines = stdout.lines().collect::<Vec<_>>();
    assert_eq!(
        lines,
        vec!["docs/components.md#Props", "docs/hooks.md", "docs/state.md"]
    );
}

#[test]
fn deps_json_outputs_structured_rows() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/tutorial.md",
        "# Tutorial\n\n[Hooks](hooks.md)\n[[components#Props]]\n[State](state.md)\n",
    );
    write_file(temp.path(), "docs/hooks.md", "# Hooks\n");
    write_file(
        temp.path(),
        "docs/components.md",
        "# Components\n\n## Props\n",
    );
    write_file(temp.path(), "docs/state.md", "# State\n");

    let output = Command::new(bin())
        .arg("deps")
        .current_dir(temp.path())
        .arg("docs/tutorial.md")
        .arg("--json")
        .output()
        .expect("run kdb deps --json");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("parse deps json");
    let rows = json.as_array().expect("deps json array");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["file"], "docs/components.md");
    assert_eq!(rows[0]["anchor"], "Props");
    assert_eq!(rows[1]["file"], "docs/hooks.md");
    assert!(rows[1]["anchor"].is_null());
    assert_eq!(rows[2]["file"], "docs/state.md");
    assert!(rows[2]["anchor"].is_null());
}

#[test]
fn deps_supports_rust_code_file_targets() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "src/lib.rs",
        "mod util;\nuse crate::core::engine::Runner;\n",
    );
    write_file(temp.path(), "src/util.rs", "pub fn helper() {}\n");
    write_file(temp.path(), "src/core/engine.rs", "pub struct Runner;\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("src/lib.rs")
        .output()
        .expect("run kdb deps for rust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["src/core/engine.rs", "src/util.rs"]
    );
}

#[test]
fn deps_supports_rust_nested_crate_roots_in_monorepos() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"apps/tool\"]\n",
    );
    write_file(
        temp.path(),
        "apps/tool/Cargo.toml",
        "[package]\nname = \"tool\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write_file(
        temp.path(),
        "apps/tool/src/lib.rs",
        "mod util;\nuse crate::core::engine::Runner;\n",
    );
    write_file(temp.path(), "apps/tool/src/util.rs", "pub fn helper() {}\n");
    write_file(
        temp.path(),
        "apps/tool/src/core/engine.rs",
        "pub struct Runner;\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("apps/tool/src/lib.rs")
        .output()
        .expect("run kdb deps for nested rust crate");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["apps/tool/src/core/engine.rs", "apps/tool/src/util.rs"]
    );
}

#[test]
fn deps_supports_rust_workspace_sibling_crate_imports() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/app\", \"crates/shared\"]\n",
    );
    write_file(
        temp.path(),
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nshared = { path = \"../shared\" }\nserde = \"1\"\n",
    );
    write_file(
        temp.path(),
        "crates/shared/Cargo.toml",
        "[package]\nname = \"shared\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write_file(
        temp.path(),
        "crates/app/src/lib.rs",
        "use serde::Serialize;\nuse shared::util::Runner;\n",
    );
    write_file(temp.path(), "crates/shared/src/lib.rs", "pub mod util;\n");
    write_file(
        temp.path(),
        "crates/shared/src/util.rs",
        "pub struct Runner;\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("crates/app/src/lib.rs")
        .output()
        .expect("run kdb deps for rust workspace sibling crate import");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["crates/shared/src/util.rs"]
    );
}

#[test]
fn deps_supports_rust_workspace_renamed_path_dependencies() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/app\", \"crates/shared\"]\n",
    );
    write_file(
        temp.path(),
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nrenamed = { package = \"shared_core\", path = \"../shared\" }\n",
    );
    write_file(
        temp.path(),
        "crates/shared/Cargo.toml",
        "[package]\nname = \"shared_core\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write_file(
        temp.path(),
        "crates/app/src/lib.rs",
        "use renamed::net::Client;\n",
    );
    write_file(temp.path(), "crates/shared/src/lib.rs", "pub mod net;\n");
    write_file(
        temp.path(),
        "crates/shared/src/net.rs",
        "pub struct Client;\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("crates/app/src/lib.rs")
        .output()
        .expect("run kdb deps for rust renamed path dependency");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["crates/shared/src/net.rs"]
    );
}

#[test]
fn deps_supports_rust_custom_lib_path_mod_declarations() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "Cargo.toml",
        "[package]\nname = \"agent\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/agent.rs\"\n",
    );
    write_file(temp.path(), "src/agent.rs", "mod db;\n");
    write_file(temp.path(), "src/db.rs", "pub struct Db;\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("src/agent.rs")
        .output()
        .expect("run kdb deps for rust custom lib path mod");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["src/db.rs"]);
}

#[test]
fn deps_supports_rust_custom_bin_path_mod_declarations() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "Cargo.toml",
        "[package]\nname = \"agent\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"worker\"\npath = \"src/worker.rs\"\n",
    );
    write_file(temp.path(), "src/worker.rs", "mod jobs;\n");
    write_file(temp.path(), "src/jobs.rs", "pub struct Jobs;\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("src/worker.rs")
        .output()
        .expect("run kdb deps for rust custom bin path mod");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["src/jobs.rs"]);
}

#[test]
fn deps_supports_rust_custom_lib_path_cross_crate_fallback() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/app\", \"crates/agent\"]\n",
    );
    write_file(
        temp.path(),
        "crates/app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nagent = { path = \"../agent\" }\n",
    );
    write_file(
        temp.path(),
        "crates/agent/Cargo.toml",
        "[package]\nname = \"agent\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/agent.rs\"\n",
    );
    write_file(temp.path(), "crates/app/src/lib.rs", "use agent::Agent;\n");
    write_file(
        temp.path(),
        "crates/agent/src/agent.rs",
        "pub struct Agent;\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("crates/app/src/lib.rs")
        .output()
        .expect("run kdb deps for rust custom lib cross-crate fallback");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["crates/agent/src/agent.rs"]
    );
}

#[test]
fn deps_supports_typescript_code_file_targets() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "web/main.ts",
        "import x from './lib';\nexport { y } from './shared/util';\nconst z = require('./cjs');\n",
    );
    write_file(temp.path(), "web/lib.ts", "export const x = 1;\n");
    write_file(temp.path(), "web/shared/util.ts", "export const y = 2;\n");
    write_file(temp.path(), "web/cjs.js", "module.exports = {};\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("web/main.ts")
        .output()
        .expect("run kdb deps for ts");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["web/cjs.js", "web/lib.ts", "web/shared/util.ts"]
    );
}

#[test]
fn deps_supports_typescript_tsconfig_path_aliases() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": {\n      \"@app/*\": [\"src/*\"]\n    }\n  }\n}\n",
    );
    write_file(
        temp.path(),
        "web/main.ts",
        "import { util } from '@app/utils';\n",
    );
    write_file(temp.path(), "src/utils.ts", "export const util = 1;\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("web/main.ts")
        .output()
        .expect("run kdb deps for tsconfig alias");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["src/utils.ts"]);
}

#[test]
fn deps_supports_typescript_monorepo_per_package_tsconfig_path_aliases() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "package.json",
        "{\n  \"name\": \"monorepo\",\n  \"private\": true,\n  \"workspaces\": [\"packages/*\"]\n}\n",
    );
    write_file(
        temp.path(),
        "packages/cli/package.json",
        "{\n  \"name\": \"@n8n/cli\"\n}\n",
    );
    write_file(
        temp.path(),
        "packages/cli/tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": {\n      \"@/*\": [\"./*\"]\n    }\n  }\n}\n",
    );
    write_file(
        temp.path(),
        "packages/cli/src/commands/start.ts",
        "import { Server } from '@/src/server';\nconst s = new Server();\n",
    );
    write_file(
        temp.path(),
        "packages/cli/src/server.ts",
        "export class Server {}\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("packages/cli/src/commands/start.ts")
        .output()
        .expect("run kdb deps for per-package tsconfig alias");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["packages/cli/src/server.ts"]
    );
}

#[test]
fn refs_resolves_typescript_monorepo_per_package_tsconfig_path_aliases() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "package.json",
        "{\n  \"name\": \"monorepo\",\n  \"private\": true,\n  \"workspaces\": [\"packages/*\"]\n}\n",
    );
    write_file(
        temp.path(),
        "packages/cli/package.json",
        "{\n  \"name\": \"@n8n/cli\"\n}\n",
    );
    write_file(
        temp.path(),
        "packages/cli/tsconfig.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": {\n      \"@/*\": [\"./*\"]\n    }\n  }\n}\n",
    );
    write_file(
        temp.path(),
        "packages/cli/src/commands/start.ts",
        "import { Server } from '@/src/server';\nconst s = new Server();\n",
    );
    write_file(
        temp.path(),
        "packages/cli/src/server.ts",
        "export class Server {}\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("refs")
        .arg("packages/cli/src/server.ts")
        .arg("-s")
        .arg("Server")
        .arg("--count")
        .output()
        .expect("run kdb refs -s --count for per-package tsconfig alias");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 2 = definition in server.ts + usage in start.ts
    assert_eq!(stdout.trim(), "2");
}

#[test]
fn deps_supports_typescript_workspace_package_imports() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "package.json",
        "{\n  \"name\": \"workspace\",\n  \"private\": true,\n  \"workspaces\": [\"packages/*\"]\n}\n",
    );
    write_file(
        temp.path(),
        "apps/web/main.ts",
        "import { Agent } from '@kernl-sdk/protocol';\nimport { run } from '@kernl-sdk/protocol/agent';\n",
    );
    write_file(
        temp.path(),
        "packages/protocol/package.json",
        "{\n  \"name\": \"@kernl-sdk/protocol\",\n  \"exports\": {\n    \".\": \"./src/index.ts\",\n    \"./agent\": \"./src/agent.ts\"\n  }\n}\n",
    );
    write_file(
        temp.path(),
        "packages/protocol/src/index.ts",
        "export const Agent = {};\n",
    );
    write_file(
        temp.path(),
        "packages/protocol/src/agent.ts",
        "export const run = () => {};\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("apps/web/main.ts")
        .output()
        .expect("run kdb deps for workspace package");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec![
            "packages/protocol/src/agent.ts",
            "packages/protocol/src/index.ts"
        ]
    );
}

#[test]
fn deps_supports_python_code_file_targets() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "app/main.py",
        "import pkg.utils\nfrom .local import helper\n",
    );
    write_file(temp.path(), "pkg/utils.py", "VALUE = 1\n");
    write_file(temp.path(), "app/local/__init__.py", "\n");
    write_file(
        temp.path(),
        "app/local/helper.py",
        "def run():\n    return 1\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("app/main.py")
        .output()
        .expect("run kdb deps for python");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec![
            "app/local/__init__.py",
            "app/local/helper.py",
            "pkg/utils.py"
        ]
    );
}

#[test]
fn deps_supports_python_pyproject_src_layout_package_imports() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "pyproject.toml",
        "[build-system]\nrequires = [\"setuptools>=61\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[tool.setuptools]\npackage-dir = {\"\" = \"src\"}\n\n[tool.setuptools.packages.find]\nwhere = [\"src\"]\n",
    );
    write_file(
        temp.path(),
        "app/main.py",
        "import mypkg.utils\nfrom mypkg.sub import thing\nimport requests\n",
    );
    write_file(temp.path(), "src/mypkg/__init__.py", "\n");
    write_file(temp.path(), "src/mypkg/utils.py", "VALUE = 1\n");
    write_file(temp.path(), "src/mypkg/sub/__init__.py", "\n");
    write_file(temp.path(), "src/mypkg/sub/thing.py", "THING = 1\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("app/main.py")
        .output()
        .expect("run kdb deps for python src layout");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec![
            "src/mypkg/sub/__init__.py",
            "src/mypkg/sub/thing.py",
            "src/mypkg/utils.py"
        ]
    );
}

#[test]
fn deps_supports_python_setup_py_src_layout_fallback() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "setup.py",
        "from setuptools import find_packages, setup\n\nsetup(\n    name=\"demo\",\n    package_dir={\"\": \"src\"},\n    packages=find_packages(where=\"src\"),\n)\n",
    );
    write_file(temp.path(), "tool/main.py", "import acme.util\n");
    write_file(temp.path(), "src/acme/__init__.py", "\n");
    write_file(temp.path(), "src/acme/util.py", "VALUE = 1\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("tool/main.py")
        .output()
        .expect("run kdb deps for python setup.py src layout");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["src/acme/util.py"]);
}

#[test]
fn deps_supports_python_poetry_src_namespace_packages() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "pyproject.toml",
        "[project]\nname = \"poetry\"\nversion = \"0.1.0\"\n\n[tool.poetry]\nrequires-poetry = \">=2.0\"\n\n[build-system]\nrequires = [\"poetry-core>=2.0\"]\nbuild-backend = \"poetry.core.masonry.api\"\n",
    );
    write_file(
        temp.path(),
        "src/poetry/factory.py",
        "from poetry.config.config import Config\n",
    );
    write_file(
        temp.path(),
        "src/poetry/config/config.py",
        "class Config:\n    pass\n",
    );

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("src/poetry/factory.py")
        .output()
        .expect("run kdb deps for poetry src namespace package");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["src/poetry/config/config.py"]
    );
}

#[test]
fn deps_python_canonicalizes_resolved_file_case_on_case_insensitive_fs() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "pyproject.toml",
        "[build-system]\nrequires = [\"setuptools>=61\"]\nbuild-backend = \"setuptools.build_meta\"\n\n[tool.setuptools]\npackage-dir = {\"\" = \"src\"}\n\n[tool.setuptools.packages.find]\nwhere = [\"src\"]\n",
    );
    write_file(
        temp.path(),
        "src/poetry/factory.py",
        "from poetry.packages.Locker import Locker\n",
    );
    write_file(
        temp.path(),
        "src/poetry/packages/locker.py",
        "class Locker:\n    pass\n",
    );
    write_file(temp.path(), "src/poetry/packages/__init__.py", "\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("src/poetry/factory.py")
        .output()
        .expect("run kdb deps for python case canonicalization");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let case_insensitive = temp.path().join("src/poetry/packages/Locker.py").is_file();

    if case_insensitive {
        assert_eq!(
            stdout.lines().collect::<Vec<_>>(),
            vec!["src/poetry/packages/locker.py"]
        );
    } else {
        assert_eq!(stdout.trim(), "(no dependencies)");
    }
}

#[test]
fn deps_supports_go_code_file_targets() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "go.mod", "module example.com/acme\n");
    write_file(
        temp.path(),
        "cmd/main.go",
        "package main\nimport (\n\t\"example.com/acme/internal/pkg\"\n\t\"./local\"\n\t\"fmt\"\n)\n",
    );
    write_file(temp.path(), "internal/pkg/a.go", "package pkg\n");
    write_file(temp.path(), "internal/pkg/b.go", "package pkg\n");
    write_file(temp.path(), "cmd/local/x.go", "package local\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("cmd/main.go")
        .output()
        .expect("run kdb deps for go");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["cmd/local/x.go", "internal/pkg/a.go", "internal/pkg/b.go"]
    );
}

#[test]
fn deps_supports_go_workspace_use_cross_module_imports() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "go.work",
        "go 1.22\n\nuse (\n\t./moda\n\t./modb\n)\n",
    );
    write_file(temp.path(), "moda/go.mod", "module example.com/moda\n");
    write_file(temp.path(), "modb/go.mod", "module example.com/modb\n");
    write_file(
        temp.path(),
        "moda/cmd/main.go",
        "package main\nimport (\n\t\"example.com/modb/pkg\"\n\t\"fmt\"\n)\n",
    );
    write_file(temp.path(), "modb/pkg/lib.go", "package pkg\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("moda/cmd/main.go")
        .output()
        .expect("run kdb deps for go workspace use");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), vec!["modb/pkg/lib.go"]);
}

#[test]
fn deps_supports_go_workspace_replace_local_path_overrides() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "go.work",
        "go 1.22\n\nuse (\n\t./moda\n\t./vendor/modb\n)\n\nreplace example.com/modb => ./fork/modb\n",
    );
    write_file(temp.path(), "moda/go.mod", "module example.com/moda\n");
    write_file(
        temp.path(),
        "vendor/modb/go.mod",
        "module example.com/modb\n",
    );
    write_file(temp.path(), "fork/modb/go.mod", "module example.com/modb\n");
    write_file(
        temp.path(),
        "moda/cmd/main.go",
        "package main\nimport \"example.com/modb/pkg\"\n",
    );
    write_file(temp.path(), "vendor/modb/pkg/original.go", "package pkg\n");
    write_file(temp.path(), "fork/modb/pkg/replaced.go", "package pkg\n");

    let output = Command::new(bin())
        .current_dir(temp.path())
        .arg("deps")
        .arg("moda/cmd/main.go")
        .output()
        .expect("run kdb deps for go workspace replace");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec!["fork/modb/pkg/replaced.go"]
    );
}

#[test]
fn graph_is_stubbed_with_clear_message() {
    let output = Command::new(bin())
        .arg("graph")
        .output()
        .expect("run kdb graph");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("`kdb graph` is not implemented yet"));
    assert!(stderr.contains("iss-0021-graph-command.md"));
}

#[test]
fn init_creates_kdb_directory_and_default_config() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();
    let expected_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("tempdir name");

    let output = Command::new(bin())
        .arg("init")
        .arg(root)
        .output()
        .expect("run kdb init");

    assert!(output.status.success());
    assert!(root.join(".kdb").is_dir());
    let config = fs::read_to_string(root.join(".kdb/config.toml")).expect("read config");
    assert!(config.contains("[workspace]"));
    assert!(config.contains(&format!("name = \"{expected_name}\"")));

    let ignore = fs::read_to_string(root.join(".kdb/ignore")).expect("read ignore");
    assert!(
        ignore.contains("target"),
        ".kdb/ignore should contain default patterns"
    );
    assert!(
        ignore.contains("node_modules"),
        ".kdb/ignore should contain node_modules"
    );
}

#[test]
fn init_errors_if_kdb_directory_already_exists() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    let output = Command::new(bin())
        .arg("init")
        .arg(temp.path())
        .output()
        .expect("run kdb init");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(".kdb already exists"));
}

#[test]
fn tasks_view_supports_show_alias_and_orders_children_by_order_key() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();

    let init = Command::new(bin())
        .arg("init")
        .arg(root)
        .output()
        .expect("run kdb init");
    assert!(init.status.success());

    fs::create_dir_all(root.join("projects/kdb")).expect("create project dir");

    let add_project = run(
        root,
        &[
            "projects",
            "add",
            "kdb",
            "--alias",
            "KDB",
            "--path",
            "projects/kdb",
        ],
    );
    assert!(add_project.status.success());

    let add_parent = run(root, &["tasks", "add", "Parent", "-P", "kdb"]);
    assert!(add_parent.status.success());
    let add_child_a = run(
        root,
        &[
            "tasks", "add", "Child A", "-P", "kdb", "--parent", "KDB-0001",
        ],
    );
    assert!(add_child_a.status.success());
    let add_child_b = run(
        root,
        &[
            "tasks", "add", "Child B", "-P", "kdb", "--parent", "KDB-0001",
        ],
    );
    assert!(add_child_b.status.success());

    let conn = Connection::open(root.join(".kdb/index.db")).expect("open sqlite db");
    conn.execute("UPDATE tasks SET \"order\" = 'b' WHERE seq = 2", [])
        .expect("set child a order");
    conn.execute("UPDATE tasks SET \"order\" = 'a' WHERE seq = 3", [])
        .expect("set child b order");

    let view_output = run(root, &["tasks", "view", "KDB-0001"]);
    assert!(view_output.status.success());
    let view_stdout = String::from_utf8_lossy(&view_output.stdout);
    assert!(view_stdout.contains("children:"));
    let child_b_at = view_stdout
        .find("KDB-0003")
        .expect("child b in view output");
    let child_a_at = view_stdout
        .find("KDB-0002")
        .expect("child a in view output");
    assert!(
        child_b_at < child_a_at,
        "expected child with lower lexical order key first"
    );

    let show_output = run(root, &["tasks", "show", "KDB-0001"]);
    assert!(show_output.status.success());
    let show_stdout = String::from_utf8_lossy(&show_output.stdout);
    assert!(show_stdout.contains("children:"));

    let json_output = run(root, &["tasks", "view", "KDB-0001", "--json"]);
    assert!(json_output.status.success());
    let payload: Value =
        serde_json::from_slice(&json_output.stdout).expect("parse tasks view json output");
    let children = payload
        .get("children")
        .and_then(|value| value.as_array())
        .expect("children array");
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].get("id").unwrap(), "KDB-0003");
    assert_eq!(children[0].get("order").unwrap(), "a");
    assert_eq!(children[1].get("id").unwrap(), "KDB-0002");
    assert_eq!(children[1].get("order").unwrap(), "b");
}

#[test]
fn tasks_delete_and_d_alias_soft_delete_to_parked() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();

    let init = Command::new(bin())
        .arg("init")
        .arg(root)
        .output()
        .expect("run kdb init");
    assert!(init.status.success());

    fs::create_dir_all(root.join("projects/kdb")).expect("create project dir");

    let add_project = run(
        root,
        &[
            "projects",
            "add",
            "kdb",
            "--alias",
            "KDB",
            "--path",
            "projects/kdb",
        ],
    );
    assert!(add_project.status.success());

    let add_one = run(root, &["tasks", "add", "Delete me", "-P", "kdb"]);
    assert!(add_one.status.success());
    let delete_one = run(root, &["tasks", "delete", "KDB-0001"]);
    assert!(delete_one.status.success());

    let first_json = run(root, &["tasks", "view", "KDB-0001", "--json"]);
    assert!(first_json.status.success());
    let first_payload: Value =
        serde_json::from_slice(&first_json.stdout).expect("parse deleted task json");
    assert_eq!(
        first_payload.get("status").and_then(|value| value.as_str()),
        Some("parked")
    );

    let add_two = run(root, &["tasks", "add", "Delete me too", "-P", "kdb"]);
    assert!(add_two.status.success());
    let delete_two = run(root, &["tasks", "d", "KDB-0002"]);
    assert!(delete_two.status.success());

    let second_json = run(root, &["tasks", "view", "KDB-0002", "--json"]);
    assert!(second_json.status.success());
    let second_payload: Value =
        serde_json::from_slice(&second_json.stdout).expect("parse alias-deleted task json");
    assert_eq!(
        second_payload
            .get("status")
            .and_then(|value| value.as_str()),
        Some("parked")
    );
}

fn titles_in_list(list_stdout: &str) -> Vec<&str> {
    list_stdout
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| l.splitn(4, "  ").nth(3))
        .map(str::trim)
        .collect()
}

fn setup_kdb_project(root: &std::path::Path) {
    let init = Command::new(bin())
        .arg("init")
        .arg(root)
        .output()
        .expect("run kdb init");
    assert!(init.status.success());
    fs::create_dir_all(root.join("projects/kdb")).expect("create project dir");
    let add_project = run(
        root,
        &[
            "projects",
            "add",
            "kdb",
            "--alias",
            "KDB",
            "--path",
            "projects/kdb",
        ],
    );
    assert!(add_project.status.success());
}

#[test]
fn tasks_move_before_reorders_project_list() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();
    setup_kdb_project(root);

    for title in ["a", "b", "c"] {
        let out = run(root, &["tasks", "add", title, "-P", "kdb"]);
        assert!(out.status.success());
    }

    // Move KDB-0003 (c) before KDB-0001 (a) → c, a, b
    let mv = run(root, &["tasks", "move", "KDB-0003", "--before", "KDB-0001"]);
    assert!(mv.status.success(), "{}", String::from_utf8_lossy(&mv.stderr));
    let listed = run(root, &["tasks", "list", "-P", "kdb"]);
    let out = String::from_utf8_lossy(&listed.stdout).into_owned();
    assert_eq!(titles_in_list(&out), vec!["c", "a", "b"]);
}

#[test]
fn tasks_move_top_and_bottom_moves_to_ends() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();
    setup_kdb_project(root);

    for title in ["a", "b", "c"] {
        assert!(run(root, &["tasks", "add", title, "-P", "kdb"]).status.success());
    }

    assert!(run(root, &["tasks", "move", "KDB-0001", "--bottom"]).status.success());
    let listed = run(root, &["tasks", "list", "-P", "kdb"]);
    let out = String::from_utf8_lossy(&listed.stdout).into_owned();
    assert_eq!(titles_in_list(&out), vec!["b", "c", "a"]);

    assert!(run(root, &["tasks", "move", "KDB-0001", "--top"]).status.success());
    let listed = run(root, &["tasks", "list", "-P", "kdb"]);
    let out = String::from_utf8_lossy(&listed.stdout).into_owned();
    assert_eq!(titles_in_list(&out), vec!["a", "b", "c"]);
}

#[test]
fn tasks_add_after_inserts_between_neighbors() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();
    setup_kdb_project(root);

    assert!(run(root, &["tasks", "add", "a", "-P", "kdb"]).status.success());
    assert!(run(root, &["tasks", "add", "b", "-P", "kdb"]).status.success());

    let insert = run(root, &["tasks", "add", "mid", "-P", "kdb", "--after", "KDB-0001"]);
    assert!(insert.status.success(), "{}", String::from_utf8_lossy(&insert.stderr));
    let listed = run(root, &["tasks", "list", "-P", "kdb"]);
    let out = String::from_utf8_lossy(&listed.stdout).into_owned();
    assert_eq!(titles_in_list(&out), vec!["a", "mid", "b"]);
}

#[test]
fn tasks_move_rejects_cross_parent_target() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path();
    setup_kdb_project(root);

    assert!(run(root, &["tasks", "add", "parent", "-P", "kdb"]).status.success());
    let child = run(
        root,
        &[
            "tasks", "add", "child", "-P", "kdb", "--parent", "KDB-0001",
        ],
    );
    assert!(child.status.success());
    assert!(run(root, &["tasks", "add", "root", "-P", "kdb"]).status.success());

    // Child (KDB-0002) has parent=KDB-0001; root task (KDB-0003) has no parent.
    let mv = run(root, &["tasks", "move", "KDB-0002", "--before", "KDB-0003"]);
    assert!(!mv.status.success());
    let stderr = String::from_utf8_lossy(&mv.stderr);
    assert!(
        stderr.contains("different parent"),
        "unexpected stderr: {stderr}"
    );
}
