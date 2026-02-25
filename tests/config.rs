use kdb::project::config::load_index_ignores;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

// -----------------------------------------------------------------------
// tests/config.rs
//
// fn write_file()                                                     L16
// fn load_index_ignores_returns_empty_when_config_missing()           L25
// fn load_index_ignores_returns_empty_when_index_section_missing()    L32
// fn load_index_ignores_reads_patterns()                              L45
// fn load_index_ignores_rejects_invalid_ignore_shape()                L58
// -----------------------------------------------------------------------

fn write_file(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write fixture file");
}

#[test]
fn load_index_ignores_returns_empty_when_config_missing() {
    let temp = tempdir().expect("tempdir");
    let patterns = load_index_ignores(temp.path()).expect("load ignores");
    assert!(patterns.is_empty());
}

#[test]
fn load_index_ignores_returns_empty_when_index_section_missing() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"demo\"\n",
    );

    let patterns = load_index_ignores(temp.path()).expect("load ignores");
    assert!(patterns.is_empty());
}

#[test]
fn load_index_ignores_reads_patterns() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"demo\"\n[index]\nignore = [\"archive/**\", \"drafts/*.md\"]\n",
    );

    let patterns = load_index_ignores(temp.path()).expect("load ignores");
    assert_eq!(patterns, vec!["archive/**", "drafts/*.md"]);
}

#[test]
fn load_index_ignores_rejects_invalid_ignore_shape() {
    let temp = tempdir().expect("tempdir");
    write_file(
        temp.path(),
        ".kdb/config.toml",
        "[project]\nname = \"demo\"\n[index]\nignore = \"archive/**\"\n",
    );

    let error = load_index_ignores(temp.path()).expect_err("invalid ignore shape");
    assert!(format!("{error:#}").contains("index.ignore"));
}
