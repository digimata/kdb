use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::index::deps::Dependency;

use super::utils::resolve_with_exts;

// --------------------------------
// src/deps/typescript.rs
//
// fn collect()                L27
// fn parse_specifiers()       L39
// fn resolve_specifier()      L66
// --------------------------------

static IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*import(?:\s+type)?(?:\s+[^"'\n]+?\s+from)?\s*["']([^"']+)["']"#)
        .expect("valid typescript import regex")
});
static EXPORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*export\s+(?:\*|\{[^}]*\})\s+from\s*["']([^"']+)["']"#)
        .expect("valid typescript export regex")
});
static REQUIRE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"require\(\s*["']([^"']+)["']\s*\)"#).expect("valid require regex")
});

pub(super) fn collect(
    root: &Path,
    source_file: &Path,
    source: &str,
    deps: &mut BTreeSet<Dependency>,
) {
    for spec in parse_specifiers(source) {
        if let Some(file) = resolve_specifier(root, source_file, &spec) {
            deps.insert(Dependency { file, anchor: None });
        }
    }
}

fn parse_specifiers(source: &str) -> BTreeSet<String> {
    let mut specs = BTreeSet::new();
    for captures in IMPORT_RE.captures_iter(source) {
        if let Some(spec) = captures
            .get(1)
            .map(|value| value.as_str().trim().to_string())
        {
            specs.insert(spec);
        }
    }
    for captures in EXPORT_RE.captures_iter(source) {
        if let Some(spec) = captures
            .get(1)
            .map(|value| value.as_str().trim().to_string())
        {
            specs.insert(spec);
        }
    }
    for captures in REQUIRE_RE.captures_iter(source) {
        if let Some(spec) = captures
            .get(1)
            .map(|value| value.as_str().trim().to_string())
        {
            specs.insert(spec);
        }
    }
    specs
}

fn resolve_specifier(root: &Path, source_file: &Path, raw_spec: &str) -> Option<PathBuf> {
    let spec = raw_spec
        .split('?')
        .next()
        .unwrap_or(raw_spec)
        .split('#')
        .next()
        .unwrap_or(raw_spec)
        .trim();
    if spec.is_empty() {
        return None;
    }

    if !spec.starts_with('.') && !spec.starts_with('/') {
        return None;
    }

    let base = if spec.starts_with('/') {
        PathBuf::from(spec.trim_start_matches('/'))
    } else {
        source_file.parent().unwrap_or(Path::new("")).join(spec)
    };

    let source_ext = source_file.extension().and_then(|value| value.to_str());
    let exts = match source_ext {
        Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => {
            ["js", "jsx", "mjs", "cjs", "ts", "tsx"].as_slice()
        }
        _ => ["ts", "tsx", "js", "jsx", "mjs", "cjs"].as_slice(),
    };

    resolve_with_exts(root, &base, exts)
}
