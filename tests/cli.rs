use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

// ----------------------------------------------------------------------
// tests/cli.rs
//
// fn write_file()                                                    L41
// fn bin()                                                           L49
// fn write_root_config()                                             L53
// fn check_exits_zero_for_clean_vault()                              L58
// fn check_exits_one_for_broken_links()                              L76
// fn check_respects_index_ignore_patterns_from_config()              L94
// fn check_orphan_only_shows_orphan_count_hint_without_listing()    L121
// fn check_orphans_flag_lists_orphan_files()                        L143
// fn check_errors_when_root_marker_missing()                        L165
// fn outline_prints_heading_tree()                                  L181
// fn outline_reports_no_headings_for_plain_markdown()               L204
// fn outline_errors_for_nonexistent_file()                          L225
// fn fmt_generates_code_index_headers_for_supported_files()         L241
// fn fmt_warns_when_nonstandard_index_rows_are_removed()            L263
// fn tree_prints_filtered_directory_structure()                     L286
// fn tree_level_option_matches_tree_l_flag()                        L316
// fn tree_json_dirs_only_and_all_flags_are_supported()              L336
// fn symbols_prints_markdown_heading_symbols()                      L362
// fn symbols_supports_public_filter_for_code_files()                L388
// fn symbols_json_outputs_structured_rows()                         L420
// fn refs_lists_inbound_references_for_file_target()                L447
// fn refs_lists_inbound_references_for_heading_target()             L474
// fn refs_count_prints_number_of_inbound_references()               L505
// fn refs_json_outputs_structured_rows()                            L526
// fn deps_lists_outbound_dependencies_for_file_target()             L553
// fn deps_json_outputs_structured_rows()                            L586
// fn deps_supports_rust_code_file_targets()                         L622
// fn deps_supports_typescript_code_file_targets()                   L648
// fn deps_supports_python_code_file_targets()                       L675
// fn deps_supports_go_code_file_targets()                           L700
// fn graph_is_stubbed_with_clear_message()                          L726
// fn init_creates_kdb_directory_and_default_config()                L740
// fn init_errors_if_kdb_directory_already_exists()                  L762
// ----------------------------------------------------------------------

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

fn write_root_config(root: &Path) {
    write_file(root, ".kdb/config.toml", "[project]\nname = \"fixture\"\n");
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
        "[project]\nname = \"fixture\"\n[index]\nignore = [\"archive/**\"]\n",
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
fn outline_prints_heading_tree() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "docs/page.md",
        "# Root\n\n## Child\n\n### Leaf\n",
    );

    let output = Command::new(bin())
        .arg("outline")
        .arg(temp.path().join("docs/page.md"))
        .output()
        .expect("run kdb outline");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("- Root"));
    assert!(stdout.contains("  - Child"));
    assert!(stdout.contains("    - Leaf"));
}

#[test]
fn outline_reports_no_headings_for_plain_markdown() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "notes/plain.md",
        "just text\nwithout headings\n",
    );

    let output = Command::new(bin())
        .arg("outline")
        .arg(temp.path().join("notes/plain.md"))
        .output()
        .expect("run kdb outline");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("(no headings)"));
}

#[test]
fn outline_errors_for_nonexistent_file() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());

    let output = Command::new(bin())
        .arg("outline")
        .arg(temp.path().join("missing.md"))
        .output()
        .expect("run kdb outline");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("file not found"));
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
fn tree_prints_filtered_directory_structure() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"fixture\"\n[index]\nignore = [\"archive/**\"]\n",
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
    assert!(stdout.lines().next().is_some_and(|line| line == "."));
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
    assert!(stdout.contains("a"));
    assert!(!stdout.contains("b"));
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
    assert_eq!(rows[0]["name"], "open");
    assert_eq!(rows[0]["public"], true);
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
fn graph_is_stubbed_with_clear_message() {
    let output = Command::new(bin())
        .arg("graph")
        .arg("--cluster")
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
    assert!(config.contains("[project]"));
    assert!(config.contains(&format!("name = \"{expected_name}\"")));
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
