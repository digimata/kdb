//! Transclusion resolution for markdown files.
//!
//! Resolves Obsidian-style `![[file#heading]]` embeds at render time.
//!
//! - [`render_file`] — read a file and resolve all embeds (for `kdb render`).
//! - [`render_content`] — resolve embeds in an in-memory string.

pub mod include;
pub mod resolve;

use std::collections::HashSet;
use std::path::Path;

pub use resolve::IncludeError;

// ------------------------------
// projects/kdb/src/render/mod.rs
//
// pub mod include             L8
// pub mod resolve             L9
// pub fn render_file()       L38
// pub fn render_content()    L47
// ------------------------------

/// Read a file from disk and resolve all `![[]]` embeds recursively.
///
/// Returns the fully rendered content with embeds replaced by their target content.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// let output = kdb::render::render_file(
///     Path::new("/vault"),
///     Path::new("notes/daily.md"),
/// );
/// ```
pub fn render_file(root: &Path, rel_path: &Path) -> Result<String, IncludeError> {
    let content = resolve::extract_section(root, rel_path, None)?;
    render_content(root, rel_path, &content)
}

/// Resolve all `![[]]` embeds in `content`.
///
/// `source_file` is the root-relative path of the file that `content` came from,
/// used for resolving relative include paths.
pub fn render_content(
    root: &Path,
    source_file: &Path,
    content: &str,
) -> Result<String, IncludeError> {
    let mut visited = HashSet::new();
    let source_key = source_file.display().to_string();
    visited.insert(source_key);
    resolve::render_content(root, source_file, content, 0, &mut visited)
}
