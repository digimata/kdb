//! Canonical ignore handling for file discovery.
//!
//! Provides the single source of truth for always-ignored directories and
//! user-configured ignore pattern compilation and matching.

use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::io::ErrorKind;
use std::path::Path;

// ----------------------------------------------------------
// kdb/src/project/ignore.rs
//
// pub const ALWAYS_IGNORED_DIR                           L30
// pub const DEFAULT_IGNORE                               L36
// pub fn load_ignore_file()                              L58
// fn parse_ignore_lines()                                L77
// pub fn build_ignore_globset()                          L95
// pub fn path_is_ignored()                              L119
// mod tests                                             L137
// fn parse_ignore_lines_wraps_bare_names()              L141
// fn parse_ignore_lines_preserves_globs()               L148
// fn parse_ignore_lines_preserves_paths_with_slash()    L155
// fn parse_ignore_lines_skips_comments_and_blanks()     L162
// fn default_ignore_parses_correctly()                  L169
// fn kdb_always_in_globset()                            L177
// ----------------------------------------------------------

/// The `.kdb` directory itself is always excluded — not user-configurable.
pub const ALWAYS_IGNORED_DIR: &str = ".kdb";

/// Default ignore patterns written to `.kdb/ignore` by `kdb init`.
///
/// These match the previously hardcoded `ALWAYS_IGNORED_DIRS` list (minus
/// `.kdb` which is handled separately).
pub const DEFAULT_IGNORE: &str = "\
# Default ignore patterns — edit to suit your project.
.git
target
node_modules
dist
build
.next
.cache
vendor
__pycache__
.venv
";

/// Load ignore patterns from `.kdb/ignore`.
///
/// Each non-empty, non-comment line becomes a pattern. Bare names (no `/` or
/// glob metacharacters) are wrapped as `**/{name}` so they match at any depth,
/// preserving the behavior of the old hardcoded directory list.
///
/// If the file does not exist, returns the same defaults that `kdb init` would
/// write, ensuring backwards compatibility for existing projects.
pub fn load_ignore_file(root: &Path) -> Result<Vec<String>> {
    let ignore_path = root.join(".kdb").join("ignore");

    let raw = match std::fs::read_to_string(&ignore_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == ErrorKind::NotFound => DEFAULT_IGNORE.to_string(),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {}", ignore_path.display()));
        }
    };

    Ok(parse_ignore_lines(&raw))
}

/// Parse gitignore-style lines into glob patterns.
///
/// Bare names (no `/` or glob metacharacters) are wrapped as `**/{name}` so
/// they match at any depth.
fn parse_ignore_lines(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            if line.contains('/') || line.contains('*') || line.contains('?') {
                line.to_string()
            } else {
                format!("**/{line}")
            }
        })
        .collect()
}

/// Compile user-supplied ignore glob patterns into a `GlobSet`.
///
/// The `.kdb` directory is always included in the compiled set regardless of
/// the input patterns.
pub fn build_ignore_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();

    // .kdb is always ignored.
    let kdb_glob = GlobBuilder::new(&format!("**/{ALWAYS_IGNORED_DIR}"))
        .literal_separator(true)
        .build()
        .context("failed to build .kdb ignore glob")?;
    builder.add(kdb_glob);

    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("invalid ignore pattern `{pattern}`"))?;
        builder.add(glob);
    }

    builder.build().context("failed to compile ignore patterns")
}

/// Check whether `rel_path` matches any pattern in the ignore set.
///
/// For directories, this also tests the path with a trailing slash.
pub fn path_is_ignored(ignore_set: &GlobSet, rel_path: &Path, is_dir: bool) -> bool {
    let slash = rel_path.to_string_lossy().replace('\\', "/");
    if slash.is_empty() {
        return false;
    }

    if ignore_set.is_match(&slash) {
        return true;
    }

    if is_dir {
        return ignore_set.is_match(format!("{slash}/"));
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ignore_lines_wraps_bare_names() {
        let input = "target\nnode_modules\n";
        let patterns = parse_ignore_lines(input);
        assert_eq!(patterns, vec!["**/target", "**/node_modules"]);
    }

    #[test]
    fn parse_ignore_lines_preserves_globs() {
        let input = "*.log\nsrc/**/tmp\n";
        let patterns = parse_ignore_lines(input);
        assert_eq!(patterns, vec!["*.log", "src/**/tmp"]);
    }

    #[test]
    fn parse_ignore_lines_preserves_paths_with_slash() {
        let input = "some/dir\n";
        let patterns = parse_ignore_lines(input);
        assert_eq!(patterns, vec!["some/dir"]);
    }

    #[test]
    fn parse_ignore_lines_skips_comments_and_blanks() {
        let input = "# comment\n\ntarget\n  # indented comment\n";
        let patterns = parse_ignore_lines(input);
        assert_eq!(patterns, vec!["**/target"]);
    }

    #[test]
    fn default_ignore_parses_correctly() {
        let patterns = parse_ignore_lines(DEFAULT_IGNORE);
        assert!(patterns.len() >= 10);
        assert!(patterns.contains(&"**/target".to_string()));
        assert!(patterns.contains(&"**/.git".to_string()));
    }

    #[test]
    fn kdb_always_in_globset() {
        let set = build_ignore_globset(&[]).unwrap();
        assert!(path_is_ignored(&set, Path::new(".kdb"), true));
        assert!(path_is_ignored(&set, Path::new("sub/.kdb"), true));
    }
}
