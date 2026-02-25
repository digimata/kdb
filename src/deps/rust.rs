use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::index::deps::Dependency;
use crate::resolve;

use super::utils::resolve_file;

// --------------------------------------
// src/deps/rust.rs
//
// pub(super) fn collect()            L24
// struct RustDependencyCollector     L33
//   fn new()                         L39
//   fn collect()                     L43
//   fn parse_use_prefix()            L64
//   fn resolve_mod_decl()            L76
//   fn resolve_use()                L102
//   fn rust_module_path()           L146
//   fn rust_file_candidates()       L163
//   fn source_segments()            L171
// --------------------------------------

pub(super) fn collect(
    root: &Path,
    source_file: &Path,
    source: &str,
    deps: &mut BTreeSet<Dependency>,
) {
    RustDependencyCollector::new(root, source_file).collect(source, deps);
}

struct RustDependencyCollector<'a> {
    root: &'a Path,
    source_file: &'a Path,
}

impl<'a> RustDependencyCollector<'a> {
    fn new(root: &'a Path, source_file: &'a Path) -> Self {
        Self { root, source_file }
    }

    fn collect(&self, source: &str, deps: &mut BTreeSet<Dependency>) {
        let (mods, uses) = resolve::collect_mod_and_use(source);

        for module_name in mods {
            let Some(file) = self.resolve_mod_decl(&module_name) else {
                continue;
            };
            deps.insert(Dependency { file, anchor: None });
        }

        for use_path in uses {
            let Some(prefix) = Self::parse_use_prefix(&use_path) else {
                continue;
            };
            let Some(file) = self.resolve_use(&prefix) else {
                continue;
            };
            deps.insert(Dependency { file, anchor: None });
        }
    }

    fn parse_use_prefix(path: &str) -> Option<String> {
        let head = path.split('{').next().unwrap_or(path).trim();
        let head = head.split(',').next().unwrap_or(head).trim();
        let head = head.split(" as ").next().unwrap_or(head).trim();
        let head = head.trim_end_matches(':').trim_end_matches(':').trim();
        if head.is_empty() {
            None
        } else {
            Some(head.to_string())
        }
    }

    fn resolve_mod_decl(&self, name: &str) -> Option<PathBuf> {
        let mut module_dir = self
            .source_file
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let file_name = self
            .source_file
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();

        if !matches!(file_name, "mod.rs" | "lib.rs" | "main.rs") {
            let stem = self.source_file.file_stem()?.to_str()?;
            module_dir.push(stem);
        }

        let base = module_dir.join(name);
        let file_candidate = base.with_extension("rs");
        if let Some(path) = resolve_file(self.root, &file_candidate) {
            return Some(path);
        }

        resolve_file(self.root, &base.join("mod.rs"))
    }

    fn resolve_use(&self, path: &str) -> Option<PathBuf> {
        let parts = path
            .split("::")
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }

        let current = self.source_segments()?;
        let mut cursor = 0usize;
        let mut module = match parts.first().copied()? {
            "crate" => {
                cursor += 1;
                Vec::new()
            }
            "self" => {
                cursor += 1;
                current
            }
            "super" => {
                let mut module = current;
                while cursor < parts.len() && parts[cursor] == "super" {
                    module.pop()?;
                    cursor += 1;
                }
                module
            }
            _ => return None,
        };

        while cursor < parts.len() {
            let part = parts[cursor];
            cursor += 1;
            if part == "self" || part == "*" {
                continue;
            }
            module.push(part.to_string());
        }

        self.rust_module_path(&module)
    }

    fn rust_module_path(&self, module: &[String]) -> Option<PathBuf> {
        if module.is_empty() {
            return None;
        }

        for size in (1..=module.len()).rev() {
            let prefix = &module[..size];
            for candidate in Self::rust_file_candidates(prefix) {
                if let Some(path) = resolve_file(self.root, &candidate) {
                    return Some(path);
                }
            }
        }

        None
    }

    fn rust_file_candidates(module: &[String]) -> [PathBuf; 2] {
        let mut base = PathBuf::from("src");
        for part in module {
            base.push(part);
        }
        [base.with_extension("rs"), base.join("mod.rs")]
    }

    fn source_segments(&self) -> Option<Vec<String>> {
        let rel = self.source_file.strip_prefix("src").ok()?;
        let file_name = rel.file_name()?.to_str()?;

        let mut segments = rel
            .parent()
            .into_iter()
            .flat_map(|path| path.iter())
            .filter_map(|part| part.to_str().map(ToString::to_string))
            .collect::<Vec<_>>();

        match file_name {
            "lib.rs" | "main.rs" | "mod.rs" => {}
            _ => {
                let stem = rel.file_stem()?.to_str()?;
                segments.push(stem.to_string());
            }
        }

        Some(segments)
    }
}
