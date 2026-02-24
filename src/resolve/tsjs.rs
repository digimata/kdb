use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use super::{
    build_line_starts, classify_local_kind, line_number_for_offset, normalize_identifier,
    resolve_workspace_specifier, sanitize_specifier, to_root_relative, ImportKind, ResolvedImport,
    WorkspacePackages,
};

// ----------------------------------------
// src/resolve/tsjs.rs
//
// static IMPORT_FROM_RE                L33
// static IMPORT_SIDE_EFFECT_RE         L38
// static EXPORT_FROM_RE                L42
// static REQUIRE_ASSIGN_RE             L49
// static REQUIRE_CALL_RE               L54
// struct ImportRequest                 L59
// pub(super) fn resolve()              L65
// fn build_resolver()                 L106
// fn collect_requests()               L131
// fn parse_import_bindings()          L221
// fn parse_import_segment()           L237
// fn parse_named_bindings()           L261
// fn parse_require_bindings()         L292
// fn parse_destructured_bindings()    L303
// fn dedupe_names()                   L338
// ----------------------------------------

static IMPORT_FROM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*import(?:\s+type)?\s+([^;\n]+?)\s+from\s*["']([^"']+)["']"#)
        .expect("valid typescript import regex")
});

static IMPORT_SIDE_EFFECT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*import\s*["']([^"']+)["']"#).expect("valid side-effect import regex")
});

static EXPORT_FROM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^\s*export\s+(?:type\s+)?(?:\*|\{[^}]*\}(?:\s+as\s+[A-Za-z_$][\w$]*)?)\s+from\s*["']([^"']+)["']"#,
    )
    .expect("valid typescript export regex")
});

static REQUIRE_ASSIGN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*(?:const|let|var)\s+([^=\n]+?)\s*=\s*require\(\s*["']([^"']+)["']\s*\)"#)
        .expect("valid require assignment regex")
});

static REQUIRE_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)require\(\s*["']([^"']+)["']\s*\)"#).expect("valid require regex")
});

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ImportRequest {
    raw: String,
    names: Vec<String>,
    line: usize,
}

pub(super) fn resolve(
    root: &Path,
    source_file: &Path,
    source: &str,
    workspace_packages: &WorkspacePackages,
) -> Vec<ResolvedImport> {
    let resolver = build_resolver();
    let source_abs = root.join(source_file);
    let mut imports = Vec::new();

    for request in collect_requests(source) {
        let Some(specifier) = sanitize_specifier(&request.raw) else {
            continue;
        };

        let mut resolved_path = resolver
            .resolve_file(&source_abs, &specifier)
            .ok()
            .and_then(|resolution| to_root_relative(root, resolution.path()));
        let kind = if resolved_path.is_some() {
            classify_local_kind(&specifier, workspace_packages)
        } else if let Some(path) = resolve_workspace_specifier(root, &specifier, workspace_packages)
        {
            resolved_path = Some(path);
            ImportKind::Workspace
        } else {
            ImportKind::External
        };

        imports.push(ResolvedImport {
            raw: request.raw,
            resolved_path,
            kind,
            names: request.names,
            line: request.line,
        });
    }

    imports
}

fn build_resolver() -> Resolver {
    let mut options = ResolveOptions::default();
    options.tsconfig = Some(TsconfigDiscovery::Auto);
    options.condition_names = vec![
        "source".to_string(),
        "import".to_string(),
        "node".to_string(),
        "default".to_string(),
    ];
    options.extensions = vec![
        ".ts".to_string(),
        ".tsx".to_string(),
        ".mts".to_string(),
        ".cts".to_string(),
        ".js".to_string(),
        ".jsx".to_string(),
        ".mjs".to_string(),
        ".cjs".to_string(),
        ".json".to_string(),
        ".node".to_string(),
    ];
    options.main_files = vec!["index".to_string()];
    Resolver::new(options)
}

fn collect_requests(source: &str) -> Vec<ImportRequest> {
    let line_starts = build_line_starts(source);
    let mut requests = Vec::new();

    for captures in IMPORT_FROM_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(bindings) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(specifier) = captures.get(2).map(|value| value.as_str().trim()) else {
            continue;
        };

        requests.push(ImportRequest {
            raw: specifier.to_string(),
            names: parse_import_bindings(bindings),
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    for captures in IMPORT_SIDE_EFFECT_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(specifier) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };

        requests.push(ImportRequest {
            raw: specifier.to_string(),
            names: Vec::new(),
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    for captures in EXPORT_FROM_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(specifier) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };

        requests.push(ImportRequest {
            raw: specifier.to_string(),
            names: Vec::new(),
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    for captures in REQUIRE_ASSIGN_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(bindings) = captures.get(1).map(|value| value.as_str()) else {
            continue;
        };
        let Some(specifier) = captures.get(2).map(|value| value.as_str().trim()) else {
            continue;
        };

        requests.push(ImportRequest {
            raw: specifier.to_string(),
            names: parse_require_bindings(bindings),
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    for captures in REQUIRE_CALL_RE.captures_iter(source) {
        let Some(full_match) = captures.get(0) else {
            continue;
        };
        let Some(specifier) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };

        requests.push(ImportRequest {
            raw: specifier.to_string(),
            names: Vec::new(),
            line: line_number_for_offset(&line_starts, full_match.start()),
        });
    }

    let mut seen = HashSet::new();
    requests.retain(|request| seen.insert(request.clone()));
    requests
}

fn parse_import_bindings(raw: &str) -> Vec<String> {
    let mut names = Vec::new();
    let binding = raw.trim().trim_start_matches("type ").trim();

    if let Some((default_part, rest)) = binding.split_once(',') {
        if let Some(name) = normalize_identifier(default_part) {
            names.push(name);
        }
        names.extend(parse_import_segment(rest));
    } else {
        names.extend(parse_import_segment(binding));
    }

    dedupe_names(names)
}

fn parse_import_segment(raw: &str) -> Vec<String> {
    let segment = raw.trim();
    if segment.is_empty() {
        return Vec::new();
    }

    if segment.starts_with('{') {
        return parse_named_bindings(segment);
    }

    if let Some(alias) = segment
        .strip_prefix('*')
        .map(str::trim)
        .and_then(|value| value.strip_prefix("as "))
        .and_then(normalize_identifier)
    {
        return vec![alias];
    }

    normalize_identifier(segment)
        .map(|value| vec![value])
        .unwrap_or_default()
}

fn parse_named_bindings(raw: &str) -> Vec<String> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    let Some((start, end)) = start.zip(end) else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }

    let inner = &raw[start + 1..end];
    let mut names = Vec::new();

    for item in inner.split(',') {
        let token = item.trim().trim_start_matches("type ").trim();
        if token.is_empty() {
            continue;
        }

        let local = token
            .split_once(" as ")
            .map(|(_, alias)| alias)
            .unwrap_or(token);
        if let Some(name) = normalize_identifier(local) {
            names.push(name);
        }
    }

    dedupe_names(names)
}

fn parse_require_bindings(raw: &str) -> Vec<String> {
    let binding = raw.trim();
    if binding.starts_with('{') {
        return parse_destructured_bindings(binding);
    }

    normalize_identifier(binding)
        .map(|name| vec![name])
        .unwrap_or_default()
}

fn parse_destructured_bindings(raw: &str) -> Vec<String> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    let Some((start, end)) = start.zip(end) else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }

    let mut names = Vec::new();
    let inner = &raw[start + 1..end];
    for token in inner.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        let local = token
            .split_once(':')
            .map(|(_, value)| value)
            .unwrap_or(token)
            .split_once('=')
            .map(|(value, _)| value)
            .unwrap_or(token)
            .trim();

        if let Some(name) = normalize_identifier(local) {
            names.push(name);
        }
    }

    dedupe_names(names)
}

fn dedupe_names(names: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for name in names {
        if seen.insert(name.clone()) {
            deduped.push(name);
        }
    }
    deduped
}
