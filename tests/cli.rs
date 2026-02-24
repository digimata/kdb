use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

// ----------------------------------------------------------------------
// tests/cli.rs
//
// fn write_file()                                                    L38
// fn bin()                                                           L46
// fn write_root_config()                                             L50
// fn check_exits_zero_for_clean_vault()                              L55
// fn check_exits_one_for_broken_links()                              L73
// fn check_respects_index_ignore_patterns_from_config()              L91
// fn check_orphan_only_shows_orphan_count_hint_without_listing()    L118
// fn check_orphans_flag_lists_orphan_files()                        L140
// fn check_errors_when_root_marker_missing()                        L162
// fn outline_prints_heading_tree()                                  L178
// fn outline_reports_no_headings_for_plain_markdown()               L201
// fn outline_errors_for_nonexistent_file()                          L222
// fn fmt_generates_code_index_headers_for_supported_files()         L238
// fn fmt_warns_when_nonstandard_index_rows_are_removed()            L260
// fn symbols_prints_markdown_heading_symbols()                      L283
// fn symbols_supports_public_filter_for_code_files()                L309
// fn symbols_json_outputs_structured_rows()                         L341
// fn refs_lists_inbound_references_for_file_target()                L368
// fn refs_lists_inbound_references_for_heading_target()             L395
// fn refs_count_prints_number_of_inbound_references()               L426
// fn refs_json_outputs_structured_rows()                            L447
// fn deps_lists_outbound_dependencies_for_file_target()             L474
// fn deps_json_outputs_structured_rows()                            L507
// fn graph_is_stubbed_with_clear_message()                          L544
// fn init_creates_kdb_directory_and_default_config()                L558
// fn init_errors_if_kdb_directory_already_exists()                  L580
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
