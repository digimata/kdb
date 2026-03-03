//! Recursive transclusion resolution engine.
//!
//! Resolves `![[file#heading]]` embeds by reading target files, extracting
//! sections via the markdown parser, and recursing into nested embeds.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::index::{parse_markdown, section_byte_bounds};
use crate::project::paths::normalize_rel_path;

use super::include::{IncludeDirective, find_embeds};

// --------------------------------------
// kdb/src/render/resolve.rs
//
// const MAX_DEPTH                    L30
// pub enum IncludeError              L34
//   fn fmt()                         L51
// pub fn resolve_include_path()      L84
// pub fn extract_section()          L116
// pub fn render_content()           L145
// fn resolve_directive()            L189
// pub fn validate_embed_target()    L219
// --------------------------------------

/// Maximum recursion depth for nested embeds.
const MAX_DEPTH: usize = 10;

/// Errors that can occur during transclusion resolution.
#[derive(Debug)]
pub enum IncludeError {
    /// The target file does not exist in the vault.
    FileNotFound(PathBuf),
    /// The target heading anchor was not found in the file.
    HeadingNotFound { file: PathBuf, anchor: String },
    /// A cycle was detected in the include chain.
    CycleDetected { chain: Vec<String> },
    /// Recursion exceeded [`MAX_DEPTH`].
    MaxDepthExceeded(usize),
    /// Failed to read a file from disk.
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl fmt::Display for IncludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(path) => {
                write!(f, "include target file not found: {}", path.display())
            }
            Self::HeadingNotFound { file, anchor } => {
                write!(
                    f,
                    "include target heading not found: {}#{}",
                    file.display(),
                    anchor
                )
            }
            Self::CycleDetected { chain } => {
                write!(f, "include cycle detected: {}", chain.join(" -> "))
            }
            Self::MaxDepthExceeded(depth) => {
                write!(f, "include depth exceeded maximum of {depth}")
            }
            Self::ReadError { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for IncludeError {}

/// Resolve an embed directive to a normalized root-relative file path.
///
/// For root-relative (`kdb://`) directives, the path is resolved from `root`.
/// Otherwise, it is resolved relative to `source_file`'s parent directory.
/// Wikilink convention: `.md` is appended if no extension is present.
pub fn resolve_include_path(
    root: &Path,
    source_file: &Path,
    directive: &IncludeDirective,
) -> Result<PathBuf, IncludeError> {
    let mut rel = PathBuf::from(&directive.file);

    // Wikilink convention: auto-append .md if no extension.
    if rel.extension().is_none() {
        rel.set_extension("md");
    }

    let candidate = if directive.root_relative {
        rel.clone()
    } else {
        let base = source_file.parent().unwrap_or(Path::new(""));
        base.join(&rel)
    };

    let normalized =
        normalize_rel_path(&candidate).ok_or_else(|| IncludeError::FileNotFound(candidate))?;

    let abs = root.join(&normalized);
    if !abs.is_file() {
        return Err(IncludeError::FileNotFound(normalized));
    }

    Ok(normalized)
}

/// Read a file and extract the section under `anchor`, or the full file if
/// `anchor` is `None`.
pub fn extract_section(
    root: &Path,
    rel_path: &Path,
    anchor: Option<&str>,
) -> Result<String, IncludeError> {
    let abs = root.join(rel_path);
    let content = fs::read_to_string(&abs).map_err(|source| IncludeError::ReadError {
        path: rel_path.to_path_buf(),
        source,
    })?;

    match anchor {
        None => Ok(content),
        Some(anchor_str) => {
            let parsed = parse_markdown(&content);
            let (start, end) = section_byte_bounds(&content, &parsed, Some(anchor_str))
                .ok_or_else(|| IncludeError::HeadingNotFound {
                    file: rel_path.to_path_buf(),
                    anchor: anchor_str.to_string(),
                })?;
            Ok(content[start..end].to_string())
        }
    }
}

/// Recursively resolve all `![[]]` embeds in `content`.
///
/// `depth` tracks current recursion level. `visited` tracks the include chain
/// for cycle detection (keyed on `"rel_path"` or `"rel_path#anchor"`).
pub fn render_content(
    root: &Path,
    source_file: &Path,
    content: &str,
    depth: usize,
    visited: &mut HashSet<String>,
) -> Result<String, IncludeError> {
    if depth > MAX_DEPTH {
        return Err(IncludeError::MaxDepthExceeded(depth));
    }

    let lines: Vec<&str> = content.lines().collect();
    let embeds = find_embeds(&lines);

    if embeds.is_empty() {
        return Ok(content.to_string());
    }

    // Build replacements bottom-to-top so line indices stay stable.
    let mut replacements: Vec<(usize, String)> = Vec::new();

    for embed in &embeds {
        let resolved = resolve_directive(root, source_file, &embed.directive, depth, visited)?;
        let trimmed = resolved.trim_end_matches('\n');
        replacements.push((embed.line, trimmed.to_string()));
    }

    replacements.reverse();

    let mut output_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    for (line_idx, replacement) in replacements {
        let replacement_lines: Vec<String> = replacement.lines().map(String::from).collect();
        output_lines.splice(line_idx..=line_idx, replacement_lines);
    }

    let mut result = output_lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

/// Resolve a single directive: read target, extract section, recurse.
fn resolve_directive(
    root: &Path,
    source_file: &Path,
    directive: &IncludeDirective,
    depth: usize,
    visited: &mut HashSet<String>,
) -> Result<String, IncludeError> {
    let target_rel = resolve_include_path(root, source_file, directive)?;

    let visit_key = match &directive.anchor {
        Some(anchor) => format!("{}#{}", target_rel.display(), anchor),
        None => target_rel.display().to_string(),
    };

    if !visited.insert(visit_key.clone()) {
        let chain: Vec<String> = visited.iter().cloned().collect();
        return Err(IncludeError::CycleDetected { chain });
    }

    let section = extract_section(root, &target_rel, directive.anchor.as_deref())?;
    let resolved = render_content(root, &target_rel, &section, depth + 1, visited)?;

    visited.remove(&visit_key);

    Ok(resolved)
}

/// Validate that an embed target exists (file + optional heading).
///
/// Used by `kdb check` to report broken embeds without rendering.
pub fn validate_embed_target(
    root: &Path,
    source_file: &Path,
    directive: &IncludeDirective,
) -> Result<(), IncludeError> {
    let target_rel = resolve_include_path(root, source_file, directive)?;

    if let Some(anchor) = &directive.anchor {
        let abs = root.join(&target_rel);
        let content = fs::read_to_string(&abs).map_err(|source| IncludeError::ReadError {
            path: target_rel.clone(),
            source,
        })?;
        let parsed = parse_markdown(&content);
        section_byte_bounds(&content, &parsed, Some(anchor)).ok_or_else(|| {
            IncludeError::HeadingNotFound {
                file: target_rel,
                anchor: anchor.clone(),
            }
        })?;
    }

    Ok(())
}
