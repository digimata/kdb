use kdb::root::{config_path, find_root};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_file(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write fixture file");
}

#[test]
fn find_root_from_nested_file_path() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"fixture\"\n",
    );
    write_file(temp.path(), "notes/sub/file.md", "# test\n");

    let nested_file = temp.path().join("notes/sub/file.md");
    let found = find_root(&nested_file).expect("find root");

    assert_eq!(found, temp.path().canonicalize().expect("canonical root"));
}

#[test]
fn find_root_when_starting_at_root_directory() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"fixture\"\n",
    );

    let found = find_root(temp.path()).expect("find root");
    assert_eq!(found, temp.path().canonicalize().expect("canonical root"));
}

#[test]
fn find_root_from_nested_directory_path() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"fixture\"\n",
    );
    fs::create_dir_all(temp.path().join("notes/sub")).expect("create nested dirs");

    let nested_dir = temp.path().join("notes/sub");
    let found = find_root(&nested_dir).expect("find root");

    assert_eq!(found, temp.path().canonicalize().expect("canonical root"));
}

#[test]
fn find_root_errors_when_marker_missing() {
    let temp = tempdir().expect("tempdir");
    let error = find_root(temp.path()).expect_err("expected missing marker error");
    let message = format!("{error:#}");
    assert!(message.contains(".kdb"));
}

#[test]
fn find_root_errors_for_nonexistent_start_path() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("does-not-exist");
    let error = find_root(&missing).expect_err("expected missing path error");
    let message = format!("{error:#}");
    assert!(message.contains("path does not exist"));
}

#[test]
fn config_path_points_to_dot_kdb_config_toml() {
    let temp = tempdir().expect("tempdir");
    let root = temp.path().canonicalize().expect("canonical root");
    assert_eq!(config_path(&root), root.join(".kdb/config.toml"));
}
