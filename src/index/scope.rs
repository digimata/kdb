use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use crate::resolve::ResolvedImport;

// ------------------------------------------
// src/index/scope.rs
//
// pub(super) struct ModuleScope          L27
//   pub fn from_imports()                L44
//   pub fn is_empty()                    L93
//   pub fn names()                       L98
//   pub fn targets()                    L103
//   pub fn definition_name()            L110
// pub(super) struct GlobSource          L117
// pub(super) struct ExportedNames       L124
//   pub fn public_names()               L133
//   pub fn has()                        L144
// pub(super) struct ReexportTarget      L150
// pub(super) struct FollowedReexport    L157
// ------------------------------------------

/// Per-file map of visible names, aliases, namespace targets, and glob sources.
/// Built from resolved imports, then enriched during the resolution loop before
/// usage scanning.
#[derive(Debug, Clone, Default)]
pub(super) struct ModuleScope {
    /// Binding name → target files where the symbol might be defined.
    pub bindings: HashMap<String, BTreeSet<PathBuf>>,
    /// Local alias → definition name (e.g. "B" → "Bar").
    pub aliases: HashMap<String, String>,
    /// Target files from namespace/dot imports whose symbols are directly in
    /// scope (Go dot imports).
    pub namespace_targets: Vec<PathBuf>,
    /// Binding names that come from namespace imports (`import * as NS`).
    pub namespace_bindings: HashSet<String>,
    /// Wildcard import sources (Rust `use foo::*`, Python `from foo import *`).
    pub glob_sources: Vec<GlobSource>,
}

impl ModuleScope {
    /// Build a scope from resolved imports, collecting glob sources for
    /// wildcard imports.
    pub fn from_imports(imports: &[ResolvedImport]) -> Self {
        let mut bindings: HashMap<String, BTreeSet<PathBuf>> = HashMap::new();
        let mut aliases = HashMap::new();
        let mut namespace_targets = BTreeSet::new();
        let mut namespace_bindings = HashSet::new();
        let mut glob_sources = Vec::new();

        for import in imports {
            let Some(target_file) = import.resolved_path.as_ref() else {
                continue;
            };
            if import.names.is_namespace {
                namespace_targets.insert(target_file.clone());
                for name in &import.names.locals {
                    namespace_bindings.insert(name.clone());
                }
            }

            // Record glob sources for wildcard imports.
            let is_glob = import.raw.ends_with(".*") || import.raw.contains("::*");
            if is_glob {
                glob_sources.push(GlobSource {
                    target_file: target_file.clone(),
                    all_filter: None,
                });
            }

            for name in &import.names.locals {
                if name.is_empty() {
                    continue;
                }
                bindings
                    .entry(name.clone())
                    .or_default()
                    .insert(target_file.clone());
            }
            aliases.extend(import.names.aliases.clone());
        }

        Self {
            bindings,
            aliases,
            namespace_targets: namespace_targets.into_iter().collect(),
            namespace_bindings,
            glob_sources,
        }
    }

    /// Check if the scope has no bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// All visible binding names.
    pub fn names(&self) -> HashSet<String> {
        self.bindings.keys().cloned().collect()
    }

    /// Target files for a binding name.
    pub fn targets(&self, name: &str) -> Option<Vec<PathBuf>> {
        self.bindings
            .get(name)
            .map(|set| set.iter().cloned().collect())
    }

    /// Return the definition name for a local alias, if one exists.
    pub fn definition_name(&self, local: &str) -> Option<&str> {
        self.aliases.get(local).map(String::as_str)
    }
}

/// A wildcard import source for glob propagation.
#[derive(Debug, Clone)]
pub(super) struct GlobSource {
    pub target_file: PathBuf,
    /// For Python: if `__all__` is defined, only these names. None = all public.
    pub all_filter: Option<Vec<String>>,
}

/// What a file exports — built from symbol_lookup + reexport_lookup.
pub(super) struct ExportedNames {
    /// Symbols defined in this file.
    pub defined: HashSet<String>,
    /// Symbols re-exported through this file.
    pub reexported: HashMap<String, ReexportTarget>,
}

impl ExportedNames {
    /// All names this file makes available (defined + re-exported).
    pub fn public_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.defined.iter().cloned().collect();
        for name in self.reexported.keys() {
            if !self.defined.contains(name) {
                names.push(name.clone());
            }
        }
        names
    }

    /// Check if a name is available from this file.
    pub fn has(&self, name: &str) -> bool {
        self.defined.contains(name) || self.reexported.contains_key(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ReexportTarget {
    pub target_file: PathBuf,
    pub definition_name: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FollowedReexport {
    pub target_file: PathBuf,
    pub lookup_name: String,
}
