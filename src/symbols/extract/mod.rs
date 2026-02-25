//! Per-language symbol extraction dispatch.

mod go;
mod python;
mod rust;
mod typescript;

// Re-export parent types so language files can `use super::{...}` unchanged.
use super::{Extractor, Symbol, SymbolKind};

// Re-export tree-sitter helpers used by language files.
use super::tree::{
    decorated_parent_or_self, extract_go_receiver_type, nearest_ancestor, normalize_type_name,
    walk_depth_first,
};

// Re-export per-language extract functions for symbols/mod.rs dispatch.
// --------------------
// src/symbols/extract/mod.rs
//
// mod go            L3
// mod python        L4
// mod rust          L5
// mod typescript    L6
// --------------------

pub(super) use self::go::extract as extract_go;
pub(super) use self::python::extract as extract_python;
pub(super) use self::rust::extract as extract_rust;
pub(super) use self::typescript::extract as extract_typescript;
