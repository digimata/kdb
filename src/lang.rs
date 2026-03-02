//! Shared code language identifiers and path-based detection.

use std::path::Path;

// ----------------------------
// qmd/src/lang.rs
//
// pub enum CodeLanguage    L15
//   pub fn from_path()     L26
//   pub fn as_str()        L40
// ----------------------------

/// Supported code languages across symbols, resolve, fmt, deps, and LSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLanguage {
    Rust,
    JavaScript,
    TypeScript,
    Tsx,
    Python,
    Go,
}

impl CodeLanguage {
    /// Determine the language from a file extension, if supported.
    pub fn from_path(path: &Path) -> Option<Self> {
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "rs" => Some(Self::Rust),
            "js" | "jsx" => Some(Self::JavaScript),
            "ts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "py" => Some(Self::Python),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    /// Human-readable language name for diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Tsx => "TSX",
            Self::Python => "Python",
            Self::Go => "Go",
        }
    }
}
