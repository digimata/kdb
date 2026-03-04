use kdb::index::VaultIndex;
use kdb::render;
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

fn write_root_config(root: &Path) {
    write_file(root, ".kdb/config.toml", "[project]\nname = \"fixture\"\n");
}

#[test]
fn render_file_no_embeds_returns_content_unchanged() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "note.md", "# Hello\n\nNo embeds here.\n");

    let output = render::render_file(root, Path::new("note.md")).unwrap();
    assert_eq!(output, "# Hello\n\nNo embeds here.\n");
}

#[test]
fn render_file_resolves_whole_file_embed() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "source.md", "included content\n");
    write_file(root, "main.md", "before\n![[source.md]]\nafter\n");

    let output = render::render_file(root, Path::new("main.md")).unwrap();
    assert_eq!(output, "before\nincluded content\nafter\n");
}

#[test]
fn render_file_resolves_heading_section() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(
        root,
        "sop.md",
        "# SOP\n\n## Setup\n\nDo the setup.\n\n## Teardown\n\nClean up.\n",
    );
    write_file(root, "main.md", "![[sop.md#setup]]\n");

    let output = render::render_file(root, Path::new("main.md")).unwrap();
    assert!(output.contains("## Setup"));
    assert!(output.contains("Do the setup."));
    assert!(!output.contains("Teardown"));
}

#[test]
fn render_file_resolves_relative_path() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "lib/snippet.md", "snippet content\n");
    write_file(root, "lib/main.md", "![[snippet.md]]\n");

    let output = render::render_file(root, Path::new("lib/main.md")).unwrap();
    assert_eq!(output, "snippet content\n");
}

#[test]
fn render_file_resolves_kdb_root_relative() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "lib/snippet.md", "root snippet\n");
    write_file(root, "docs/main.md", "![[kdb://lib/snippet.md]]\n");

    let output = render::render_file(root, Path::new("docs/main.md")).unwrap();
    assert_eq!(output, "root snippet\n");
}

#[test]
fn render_file_resolves_wikilink_without_extension() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "glossary.md", "definitions here\n");
    write_file(root, "main.md", "![[glossary]]\n");

    let output = render::render_file(root, Path::new("main.md")).unwrap();
    assert_eq!(output, "definitions here\n");
}

#[test]
fn render_file_recursive_embeds() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "c.md", "leaf\n");
    write_file(root, "b.md", "![[c.md]]\n");
    write_file(root, "a.md", "![[b.md]]\n");

    let output = render::render_file(root, Path::new("a.md")).unwrap();
    assert_eq!(output, "leaf\n");
}

#[test]
fn render_file_cycle_detected() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "a.md", "![[b.md]]\n");
    write_file(root, "b.md", "![[a.md]]\n");

    let result = render::render_file(root, Path::new("a.md"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cycle"), "expected cycle error, got: {err}");
}

#[test]
fn render_file_missing_target() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "main.md", "![[nonexistent.md]]\n");

    let result = render::render_file(root, Path::new("main.md"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "expected not found error, got: {err}"
    );
}

#[test]
fn render_file_missing_heading() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "sop.md", "# SOP\n\n## Setup\n\nContent.\n");
    write_file(root, "main.md", "![[sop.md#nonexistent]]\n");

    let result = render::render_file(root, Path::new("main.md"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("heading not found"),
        "expected heading not found, got: {err}"
    );
}

#[test]
fn render_file_multiple_embeds_in_one_file() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "a.md", "alpha\n");
    write_file(root, "b.md", "beta\n");
    write_file(root, "main.md", "start\n![[a.md]]\nmiddle\n![[b.md]]\nend\n");

    let output = render::render_file(root, Path::new("main.md")).unwrap();
    assert_eq!(output, "start\nalpha\nmiddle\nbeta\nend\n");
}

#[test]
fn render_content_inline_embeds_not_on_own_line_are_ignored() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "a.md", "content\n");

    let input = "see ![[a.md]] for details\n";
    let output = render::render_content(root, Path::new("main.md"), input).unwrap();
    assert_eq!(output, input, "inline embeds should not be resolved");
}

#[test]
fn check_reports_broken_embed() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "main.md", "# Main\n\n![[missing.md]]\n");

    let index = VaultIndex::build(root).unwrap();
    let report = index.check();

    assert!(!report.broken_embeds.is_empty(), "expected broken embed");
    assert_eq!(report.broken_embeds[0].source_file.to_str().unwrap(), "main.md");
    assert!(report.broken_embeds[0].reason.contains("not found"));
}

#[test]
fn check_reports_broken_embed_heading() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "sop.md", "# SOP\n\n## Real\n\nContent.\n");
    write_file(root, "main.md", "# Main\n\n![[sop.md#fake]]\n");

    let index = VaultIndex::build(root).unwrap();
    let report = index.check();

    assert!(!report.broken_embeds.is_empty(), "expected broken embed");
    assert!(report.broken_embeds[0].reason.contains("heading not found"));
}

#[test]
fn check_valid_embed_no_errors() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(root, "sop.md", "# SOP\n\n## Setup\n\nContent.\n");
    write_file(root, "main.md", "# Main\n\n![[sop.md#setup]]\n");

    let index = VaultIndex::build(root).unwrap();
    let report = index.check();

    assert!(report.broken_embeds.is_empty(), "expected no broken embeds");
}

#[test]
fn render_file_skips_embeds_in_code_blocks() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(
        root,
        "main.md",
        "# Example\n\n```markdown\n![[nonexistent.md]]\n```\n",
    );

    let output = render::render_file(root, Path::new("main.md")).unwrap();
    assert!(
        output.contains("![[nonexistent.md]]"),
        "embed inside code block should be preserved"
    );
}

#[test]
fn check_ignores_embeds_in_code_blocks() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    write_root_config(root);
    write_file(
        root,
        "main.md",
        "# Example\n\n```markdown\n![[nonexistent.md]]\n```\n",
    );

    let index = VaultIndex::build(root).unwrap();
    let report = index.check();

    assert!(
        report.broken_embeds.is_empty(),
        "embeds inside code blocks should not be checked"
    );
}
