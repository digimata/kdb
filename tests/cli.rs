use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

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
fn check_orphan_only_reports_orphans_without_broken_links() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\nNo links here.\n");
    write_file(temp.path(), "b.md", "# B\n\nNo links here either.\n");

    let output = Command::new(bin())
        .arg("check")
        .arg(temp.path())
        .output()
        .expect("run kdb check");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("orphan file"));
    assert!(!stdout.contains("broken link"));
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
