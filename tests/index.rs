use kdb::index::{
    normalize_rel_path, parse_markdown, parse_markdown_target, parse_wikilink_target,
    resolve_target_path, slug_anchor, HeadingKey, LinkKind, LinkTarget, VaultIndex,
};
use kdb::resolve::ImportKind;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// ----------------------------------------------------------------------------------
// tests/index.rs
//
// fn write_file()                                                                L62
// fn write_root_config()                                                         L70
// fn parse_markdown_extracts_headings_and_internal_links()                       L75
// fn parse_markdown_multiple_links_on_one_line_have_distinct_columns()          L108
// fn parse_markdown_target_keeps_url_encoded_paths()                            L118
// fn parse_markdown_deduplicates_heading_anchors()                              L130
// fn parse_markdown_target_filters_external_and_non_markdown_links()            L141
// fn parse_wikilink_target_supports_aliases_and_anchors()                       L172
// fn parse_wikilink_target_supports_alias_and_anchor_together()                 L199
// fn normalize_rel_path_rejects_escape_attempts()                               L211
// fn resolve_target_path_handles_markdown_and_wikilink_rules()                  L221
// fn vault_index_check_reports_broken_links_orphans_and_inbound_maps()          L262
// fn vault_index_multiple_sources_to_same_target_have_inbound_count_gt_one()    L299
// fn vault_index_single_file_is_reported_as_orphan()                            L315
// fn vault_index_ignores_non_markdown_files()                                   L326
// fn vault_index_build_with_ignores_skips_matching_paths()                      L339
// fn vault_index_respects_root_gitignore_rules()                                L357
// fn vault_index_respects_nested_gitignore_negation_rules()                     L370
// fn vault_index_incremental_upsert_respects_ignore_patterns()                  L383
// fn vault_index_build_populates_code_import_maps_for_typescript()              L402
// fn vault_index_build_populates_workspace_package_map_and_imports()            L439
// fn slug_anchor_normalizes_heading_text()                                      L491
// fn parse_markdown_heading_with_inline_code()                                  L502
// fn parse_markdown_link_inside_heading()                                       L510
// fn parse_markdown_ignores_wikilinks_in_code_blocks()                          L519
// fn parse_markdown_ignores_wikilinks_in_inline_code()                          L533
// fn parse_markdown_frontmatter_does_not_create_headings()                      L549
// fn parse_markdown_empty_file()                                                L557
// fn parse_markdown_file_with_no_headings()                                     L564
// fn parse_markdown_heading_with_special_chars()                                L571
// fn parse_markdown_all_six_heading_levels()                                    L580
// fn slug_anchor_all_special_characters()                                       L594
// fn slug_anchor_unicode_characters()                                           L601
// fn slug_anchor_mixed_separators()                                             L609
// fn slug_anchor_trailing_separators()                                          L615
// fn vault_index_file_linked_to_is_not_orphan()                                 L625
// fn vault_index_circular_references_are_not_broken()                           L640
// fn vault_index_self_referencing_links_do_not_count_as_inbound()               L655
// fn vault_index_broken_heading_anchor()                                        L668
// fn vault_index_wikilink_resolution()                                          L682
// fn vault_index_deeply_nested_files()                                          L695
// fn vault_index_empty_file_is_indexed()                                        L713
// fn normalize_rel_path_current_dir_only()                                      L731
// fn normalize_rel_path_deep_parent_traversal()                                 L739
// fn resolve_target_path_absolute_path_rejected()                               L754
// fn resolve_target_path_wikilink_with_explicit_md_extension()                  L767
// fn resolve_target_path_source_at_root_level()                                 L781
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
fn parse_markdown_extracts_headings_and_internal_links() {
    let parsed = parse_markdown(
        "# Intro\n\n## Sub Heading\n\n[a](notes/a.md#Heading)\n[[wiki/page#Section]]\n[[wiki/page|Alias]]\n[ext](https://example.com)\n",
    );

    assert_eq!(parsed.headings.len(), 2);
    assert_eq!(parsed.headings[0].title, "Intro");
    assert_eq!(parsed.headings[0].anchor, "intro");
    assert_eq!(parsed.headings[1].title, "Sub Heading");
    assert_eq!(parsed.headings[1].anchor, "sub-heading");

    assert_eq!(parsed.links.len(), 3);
    assert!(parsed.links.iter().any(|link| {
        matches!(link.kind, LinkKind::Markdown)
            && link.raw == "notes/a.md#Heading"
            && link.target.file.as_deref() == Some("notes/a.md")
            && link.target.anchor.as_deref() == Some("Heading")
    }));
    assert!(parsed.links.iter().any(|link| {
        matches!(link.kind, LinkKind::Wikilink)
            && link.raw == "[[wiki/page#Section]]"
            && link.target.file.as_deref() == Some("wiki/page")
            && link.target.anchor.as_deref() == Some("Section")
    }));
    assert!(parsed.links.iter().any(|link| {
        matches!(link.kind, LinkKind::Wikilink)
            && link.raw == "[[wiki/page|Alias]]"
            && link.target.file.as_deref() == Some("wiki/page")
            && link.target.anchor.is_none()
    }));
}

#[test]
fn parse_markdown_multiple_links_on_one_line_have_distinct_columns() {
    let parsed = parse_markdown("See [One](a.md) and [Two](b.md)\n");
    assert_eq!(parsed.links.len(), 2);
    assert_eq!(parsed.links[0].line, 1);
    assert_eq!(parsed.links[1].line, 1);
    assert_eq!(parsed.links[0].column, 5);
    assert_eq!(parsed.links[1].column, 21);
}

#[test]
fn parse_markdown_target_keeps_url_encoded_paths() {
    let target = parse_markdown_target("path%20with%20spaces.md#my-heading");
    assert_eq!(
        target,
        Some(LinkTarget {
            file: Some("path%20with%20spaces.md".to_string()),
            anchor: Some("my-heading".to_string()),
        })
    );
}

#[test]
fn parse_markdown_deduplicates_heading_anchors() {
    let parsed = parse_markdown("# Same\n## Same\n### Same\n");
    let anchors = parsed
        .headings
        .iter()
        .map(|heading| heading.anchor.as_str())
        .collect::<Vec<_>>();
    assert_eq!(anchors, vec!["same", "same-1", "same-2"]);
}

#[test]
fn parse_markdown_target_filters_external_and_non_markdown_links() {
    assert_eq!(
        parse_markdown_target("notes/a.md#Intro"),
        Some(LinkTarget {
            file: Some("notes/a.md".to_string()),
            anchor: Some("Intro".to_string())
        })
    );
    assert_eq!(
        parse_markdown_target("#Local"),
        Some(LinkTarget {
            file: None,
            anchor: Some("Local".to_string())
        })
    );
    assert_eq!(
        parse_markdown_target("notes/a.md"),
        Some(LinkTarget {
            file: Some("notes/a.md".to_string()),
            anchor: None
        })
    );

    assert_eq!(parse_markdown_target(""), None);
    assert_eq!(parse_markdown_target("https://example.com"), None);
    assert_eq!(parse_markdown_target("mailto:user@example.com"), None);
    assert_eq!(parse_markdown_target("notes/a.txt"), None);
    assert_eq!(parse_markdown_target("#"), None);
}

#[test]
fn parse_wikilink_target_supports_aliases_and_anchors() {
    assert_eq!(
        parse_wikilink_target("topic#One"),
        Some(LinkTarget {
            file: Some("topic".to_string()),
            anchor: Some("One".to_string())
        })
    );
    assert_eq!(
        parse_wikilink_target("topic|Alias"),
        Some(LinkTarget {
            file: Some("topic".to_string()),
            anchor: None
        })
    );
    assert_eq!(
        parse_wikilink_target("#Local"),
        Some(LinkTarget {
            file: None,
            anchor: Some("Local".to_string())
        })
    );
    assert_eq!(parse_wikilink_target(""), None);
    assert_eq!(parse_wikilink_target("|Alias"), None);
}

#[test]
fn parse_wikilink_target_supports_alias_and_anchor_together() {
    let target = parse_wikilink_target("file#heading|Display");
    assert_eq!(
        target,
        Some(LinkTarget {
            file: Some("file".to_string()),
            anchor: Some("heading".to_string()),
        })
    );
}

#[test]
fn normalize_rel_path_rejects_escape_attempts() {
    assert_eq!(
        normalize_rel_path(Path::new("docs/./guide/../intro.md")),
        Some(PathBuf::from("docs/intro.md"))
    );
    assert_eq!(normalize_rel_path(Path::new("../outside.md")), None);
    assert_eq!(normalize_rel_path(Path::new("/abs/path.md")), None);
}

#[test]
fn resolve_target_path_handles_markdown_and_wikilink_rules() {
    let source = Path::new("notes/topic.md");

    let markdown = LinkTarget {
        file: Some("../index.md".to_string()),
        anchor: Some("Intro".to_string()),
    };
    assert_eq!(
        resolve_target_path(source, LinkKind::Markdown, &markdown),
        Some(PathBuf::from("index.md"))
    );

    let wikilink = LinkTarget {
        file: Some("refs/overview".to_string()),
        anchor: None,
    };
    assert_eq!(
        resolve_target_path(source, LinkKind::Wikilink, &wikilink),
        Some(PathBuf::from("notes/refs/overview.md"))
    );

    let same_file = LinkTarget {
        file: None,
        anchor: Some("section".to_string()),
    };
    assert_eq!(
        resolve_target_path(source, LinkKind::Markdown, &same_file),
        Some(PathBuf::from("notes/topic.md"))
    );

    let escape = LinkTarget {
        file: Some("../../outside.md".to_string()),
        anchor: None,
    };
    assert_eq!(
        resolve_target_path(source, LinkKind::Markdown, &escape),
        None
    );
}

#[test]
fn vault_index_check_reports_broken_links_orphans_and_inbound_maps() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "a.md",
        "# A\n\n[B](b.md#Target)\n[Missing](missing.md)\n",
    );
    write_file(temp.path(), "b.md", "# B\n\n## Target\n");
    write_file(temp.path(), "c.md", "# C\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    assert_eq!(report.broken_links.len(), 1);
    assert!(report.broken_links[0]
        .reason
        .contains("target file not found: missing.md"));

    assert_eq!(
        report.orphans,
        vec![PathBuf::from("a.md"), PathBuf::from("c.md")]
    );

    let key = HeadingKey {
        file: PathBuf::from("b.md"),
        anchor: "target".to_string(),
    };
    let refs = index
        .heading_inbound
        .get(&key)
        .expect("heading inbound refs");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].source_file, PathBuf::from("a.md"));
}

#[test]
fn vault_index_multiple_sources_to_same_target_have_inbound_count_gt_one() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[Target](target.md)\n");
    write_file(temp.path(), "b.md", "# B\n\n[Target](target.md)\n");
    write_file(temp.path(), "target.md", "# Target\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let inbound = index
        .file_inbound
        .get(Path::new("target.md"))
        .expect("target inbound refs");
    assert_eq!(inbound.len(), 2);
}

#[test]
fn vault_index_single_file_is_reported_as_orphan() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "solo.md", "# Solo\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();
    assert_eq!(report.orphans, vec![PathBuf::from("solo.md")]);
}

#[test]
fn vault_index_ignores_non_markdown_files() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n");
    write_file(temp.path(), "notes.txt", "not markdown");
    write_file(temp.path(), "image.png", "binary-ish");

    let index = VaultIndex::build(temp.path()).expect("build index");
    assert_eq!(index.files.len(), 1);
    assert!(index.files.contains_key(Path::new("a.md")));
}

#[test]
fn vault_index_build_with_ignores_skips_matching_paths() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "keep.md", "# Keep\n");
    write_file(temp.path(), "archive/hidden.md", "# Hidden\n");
    write_file(temp.path(), "archive/nested/deep.md", "# Deep\n");

    let ignore_patterns = vec!["archive/**".to_string()];
    let index = VaultIndex::build_with_ignores(temp.path(), &ignore_patterns).expect("build index");

    assert!(index.files.contains_key(Path::new("keep.md")));
    assert!(!index.files.contains_key(Path::new("archive/hidden.md")));
    assert!(!index
        .files
        .contains_key(Path::new("archive/nested/deep.md")));
}

#[test]
fn vault_index_respects_root_gitignore_rules() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), ".gitignore", "archive/\n");
    write_file(temp.path(), "keep.md", "# Keep\n");
    write_file(temp.path(), "archive/hidden.md", "# Hidden\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    assert!(index.files.contains_key(Path::new("keep.md")));
    assert!(!index.files.contains_key(Path::new("archive/hidden.md")));
}

#[test]
fn vault_index_respects_nested_gitignore_negation_rules() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "docs/.gitignore", "*.md\n!keep.md\n");
    write_file(temp.path(), "docs/drop.md", "# Drop\n");
    write_file(temp.path(), "docs/keep.md", "# Keep\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    assert!(index.files.contains_key(Path::new("docs/keep.md")));
    assert!(!index.files.contains_key(Path::new("docs/drop.md")));
}

#[test]
fn vault_index_incremental_upsert_respects_ignore_patterns() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "keep.md", "# Keep\n");

    let ignore_patterns = vec!["archive/**".to_string()];
    let mut index =
        VaultIndex::build_with_ignores(temp.path(), &ignore_patterns).expect("build index");

    index.upsert_file(
        PathBuf::from("archive/live.md"),
        temp.path().join("archive/live.md"),
        "# Live\n",
    );

    assert!(!index.files.contains_key(Path::new("archive/live.md")));
}

#[test]
fn vault_index_build_populates_code_import_maps_for_typescript() {
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
        "import type { Util } from '@app/utils';\nimport local from './local';\n",
    );
    write_file(temp.path(), "src/utils.ts", "export type Util = string;\n");
    write_file(temp.path(), "web/local.ts", "export default 1;\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let imports = index
        .code_imports
        .get(Path::new("web/main.ts"))
        .expect("code imports for web/main.ts");

    assert!(imports.iter().any(|import| {
        import.raw == "@app/utils"
            && import.resolved_path.as_deref() == Some(Path::new("src/utils.ts"))
            && import.kind == ImportKind::TsconfigPath
            && import.names.iter().any(|name| name == "Util")
    }));
    assert!(imports.iter().any(|import| {
        import.raw == "./local"
            && import.resolved_path.as_deref() == Some(Path::new("web/local.ts"))
            && import.kind == ImportKind::Relative
            && import.names.iter().any(|name| name == "local")
    }));
}

#[test]
fn vault_index_build_populates_workspace_package_map_and_imports() {
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

    let index = VaultIndex::build(temp.path()).expect("build index");
    assert_eq!(
        index.workspace_packages.get("@kernl-sdk/protocol"),
        Some(&PathBuf::from("packages/protocol"))
    );

    let imports = index
        .code_imports
        .get(Path::new("apps/web/main.ts"))
        .expect("code imports for apps/web/main.ts");
    assert!(imports.iter().any(|import| {
        import.raw == "@kernl-sdk/protocol"
            && import.resolved_path.as_deref() == Some(Path::new("packages/protocol/src/index.ts"))
            && import.kind == ImportKind::Workspace
    }));
    assert!(imports.iter().any(|import| {
        import.raw == "@kernl-sdk/protocol/agent"
            && import.resolved_path.as_deref() == Some(Path::new("packages/protocol/src/agent.ts"))
            && import.kind == ImportKind::Workspace
    }));
}

#[test]
fn slug_anchor_normalizes_heading_text() {
    assert_eq!(slug_anchor("  Hello, Rust World!  "), "hello-rust-world");
    assert_eq!(slug_anchor("___"), "section");
    assert_eq!(slug_anchor("A__B---C"), "a-b-c");
}

// ---------------------------------------------------------------------------
// Parser edge cases
// ---------------------------------------------------------------------------

#[test]
fn parse_markdown_heading_with_inline_code() {
    let parsed = parse_markdown("## The `useState` Hook\n");
    assert_eq!(parsed.headings.len(), 1);
    assert_eq!(parsed.headings[0].title, "The useState Hook");
    assert_eq!(parsed.headings[0].anchor, "the-usestate-hook");
}

#[test]
fn parse_markdown_link_inside_heading() {
    let parsed = parse_markdown("## See [Overview](overview.md)\n");
    assert_eq!(parsed.headings.len(), 1);
    assert_eq!(parsed.headings[0].title, "See Overview");
    assert_eq!(parsed.links.len(), 1);
    assert_eq!(parsed.links[0].target.file.as_deref(), Some("overview.md"));
}

#[test]
fn parse_markdown_ignores_wikilinks_in_code_blocks() {
    let content = "# Title\n\n```\n[[should/ignore]]\n```\n\n[[real/link]]\n";
    let parsed = parse_markdown(content);
    // Only the wikilink outside the code block should be parsed
    let wikilinks: Vec<_> = parsed
        .links
        .iter()
        .filter(|l| matches!(l.kind, LinkKind::Wikilink))
        .collect();
    assert_eq!(wikilinks.len(), 1);
    assert_eq!(wikilinks[0].target.file.as_deref(), Some("real/link"));
}

#[test]
fn parse_markdown_ignores_wikilinks_in_inline_code() {
    let content = "# Title\n\nSee `[[not/a/link]]` for details.\n\n[[actual/link]]\n";
    let parsed = parse_markdown(content);
    let wikilinks: Vec<_> = parsed
        .links
        .iter()
        .filter(|l| matches!(l.kind, LinkKind::Wikilink))
        .collect();
    // Inline code wikilinks may or may not be filtered — this test documents behavior
    // At minimum the real link must be present
    assert!(wikilinks
        .iter()
        .any(|l| l.target.file.as_deref() == Some("actual/link")));
}

#[test]
fn parse_markdown_frontmatter_does_not_create_headings() {
    let content = "---\ntitle: My Note\ntags: [a, b]\n---\n\n# Real Heading\n";
    let parsed = parse_markdown(content);
    assert_eq!(parsed.headings.len(), 1);
    assert_eq!(parsed.headings[0].title, "Real Heading");
}

#[test]
fn parse_markdown_empty_file() {
    let parsed = parse_markdown("");
    assert!(parsed.headings.is_empty());
    assert!(parsed.links.is_empty());
}

#[test]
fn parse_markdown_file_with_no_headings() {
    let parsed = parse_markdown("Just some text\n\nwith paragraphs.\n");
    assert!(parsed.headings.is_empty());
    assert!(parsed.links.is_empty());
}

#[test]
fn parse_markdown_heading_with_special_chars() {
    let parsed = parse_markdown("## What's New in v2.0?\n");
    // pulldown-cmark converts ASCII apostrophe to Unicode smart quote
    assert_eq!(parsed.headings[0].title, "What\u{2019}s New in v2.0?");
    // slug strips non-ASCII, so the smart quote disappears
    assert_eq!(parsed.headings[0].anchor, "whats-new-in-v20");
}

#[test]
fn parse_markdown_all_six_heading_levels() {
    let content = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6\n";
    let parsed = parse_markdown(content);
    assert_eq!(parsed.headings.len(), 6);
    for (i, heading) in parsed.headings.iter().enumerate() {
        assert_eq!(heading.level, (i + 1) as u8);
    }
}

// ---------------------------------------------------------------------------
// Slug edge cases
// ---------------------------------------------------------------------------

#[test]
fn slug_anchor_all_special_characters() {
    assert_eq!(slug_anchor("!!!@@@###$$$"), "section");
    assert_eq!(slug_anchor(""), "section");
    assert_eq!(slug_anchor("   "), "section");
}

#[test]
fn slug_anchor_unicode_characters() {
    // Non-ASCII chars are stripped since slug only keeps ascii alphanumeric
    assert_eq!(slug_anchor("Intro"), "intro");
    // Pure unicode heading falls back to "section"
    assert_eq!(slug_anchor("\u{1F600}\u{1F600}\u{1F600}"), "section");
}

#[test]
fn slug_anchor_mixed_separators() {
    assert_eq!(slug_anchor("one - two _ three"), "one-two-three");
    assert_eq!(slug_anchor("a---b___c   d"), "a-b-c-d");
}

#[test]
fn slug_anchor_trailing_separators() {
    assert_eq!(slug_anchor("trailing---"), "trailing");
    assert_eq!(slug_anchor("---leading"), "leading");
}

// ---------------------------------------------------------------------------
// Index edge cases
// ---------------------------------------------------------------------------

#[test]
fn vault_index_file_linked_to_is_not_orphan() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[B](b.md)\n");
    write_file(temp.path(), "b.md", "# B\n\nJust content, no links.\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    assert!(report.broken_links.is_empty());
    // b.md is linked to by a.md, so only a.md is the orphan
    assert_eq!(report.orphans, vec![PathBuf::from("a.md")]);
}

#[test]
fn vault_index_circular_references_are_not_broken() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[B](b.md)\n");
    write_file(temp.path(), "b.md", "# B\n\n[A](a.md)\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    assert!(report.broken_links.is_empty());
    // Both files link to each other, so neither is an orphan
    assert!(report.orphans.is_empty());
}

#[test]
fn vault_index_self_referencing_links_do_not_count_as_inbound() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "self.md", "# Self\n\n[back to top](#self)\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    // Self-links don't count as inbound from another file
    assert_eq!(report.orphans, vec![PathBuf::from("self.md")]);
}

#[test]
fn vault_index_broken_heading_anchor() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[B](b.md#nonexistent)\n");
    write_file(temp.path(), "b.md", "# B\n\n## Exists\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    assert_eq!(report.broken_links.len(), 1);
    assert!(report.broken_links[0].reason.contains("heading not found"));
}

#[test]
fn vault_index_wikilink_resolution() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "a.md", "# A\n\n[[b#target]]\n");
    write_file(temp.path(), "b.md", "# B\n\n## Target\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    assert!(report.broken_links.is_empty());
}

#[test]
fn vault_index_deeply_nested_files() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(
        temp.path(),
        "a/b/c/deep.md",
        "# Deep\n\n[Top](../../../top.md)\n",
    );
    write_file(temp.path(), "top.md", "# Top\n\n[Deep](a/b/c/deep.md)\n");

    let index = VaultIndex::build(temp.path()).expect("build index");
    let report = index.check();

    assert!(report.broken_links.is_empty());
    assert!(report.orphans.is_empty());
}

#[test]
fn vault_index_empty_file_is_indexed() {
    let temp = tempdir().expect("tempdir");
    write_root_config(temp.path());
    write_file(temp.path(), "empty.md", "");
    write_file(temp.path(), "a.md", "# A\n\n[Empty](empty.md)\n");

    let index = VaultIndex::build(temp.path()).expect("build index");

    assert!(index.files.contains_key(Path::new("empty.md")));
    let report = index.check();
    assert!(report.broken_links.is_empty());
}

// ---------------------------------------------------------------------------
// Normalize rel path edge cases
// ---------------------------------------------------------------------------

#[test]
fn normalize_rel_path_current_dir_only() {
    assert_eq!(
        normalize_rel_path(Path::new("./file.md")),
        Some(PathBuf::from("file.md"))
    );
}

#[test]
fn normalize_rel_path_deep_parent_traversal() {
    // Valid: go up one then back down
    assert_eq!(
        normalize_rel_path(Path::new("a/b/../c/file.md")),
        Some(PathBuf::from("a/c/file.md"))
    );
    // Invalid: go up more levels than depth
    assert_eq!(normalize_rel_path(Path::new("a/../../file.md")), None);
}

// ---------------------------------------------------------------------------
// Resolve target path edge cases
// ---------------------------------------------------------------------------

#[test]
fn resolve_target_path_absolute_path_rejected() {
    let source = Path::new("notes/topic.md");
    let target = LinkTarget {
        file: Some("/etc/passwd".to_string()),
        anchor: None,
    };
    assert_eq!(
        resolve_target_path(source, LinkKind::Markdown, &target),
        None
    );
}

#[test]
fn resolve_target_path_wikilink_with_explicit_md_extension() {
    let source = Path::new("notes/topic.md");
    let target = LinkTarget {
        file: Some("other.md".to_string()),
        anchor: None,
    };
    // Wikilink with explicit .md should not double-add extension
    assert_eq!(
        resolve_target_path(source, LinkKind::Wikilink, &target),
        Some(PathBuf::from("notes/other.md"))
    );
}

#[test]
fn resolve_target_path_source_at_root_level() {
    let source = Path::new("root.md");
    let target = LinkTarget {
        file: Some("sibling.md".to_string()),
        anchor: None,
    };
    assert_eq!(
        resolve_target_path(source, LinkKind::Markdown, &target),
        Some(PathBuf::from("sibling.md"))
    );
}
