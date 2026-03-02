use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::index::deps::Dependency;
use crate::resolve;

use super::utils::resolve_with_exts;

// -----------------------------------
// qmd/src/deps/typescript.rs
//
// pub(super) fn collect()         L22
// struct TsDependencyCollector    L31
//   fn new()                      L37
//   fn collect()                  L41
//   fn resolve_specifier()        L50
//   fn preferred_exts()           L79
// const JS_FIRST                  L80
// const TS_FIRST                  L81
// -----------------------------------

pub(super) fn collect(
    root: &Path,
    source_file: &Path,
    source: &str,
    deps: &mut BTreeSet<Dependency>,
) {
    TsDependencyCollector::new(root, source_file).collect(source, deps);
}

struct TsDependencyCollector<'a> {
    root: &'a Path,
    source_file: &'a Path,
}

impl<'a> TsDependencyCollector<'a> {
    fn new(root: &'a Path, source_file: &'a Path) -> Self {
        Self { root, source_file }
    }

    fn collect(&self, source: &str, deps: &mut BTreeSet<Dependency>) {
        for specifier in resolve::collect_specifiers(self.source_file, source) {
            let Some(file) = self.resolve_specifier(&specifier) else {
                continue;
            };
            deps.insert(Dependency { file, anchor: None });
        }
    }

    fn resolve_specifier(&self, raw_spec: &str) -> Option<PathBuf> {
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
            self.source_file
                .parent()
                .unwrap_or(Path::new(""))
                .join(spec)
        };

        resolve_with_exts(self.root, &base, self.preferred_exts())
    }

    fn preferred_exts(&self) -> &'static [&'static str] {
        const JS_FIRST: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "tsx"];
        const TS_FIRST: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

        match self
            .source_file
            .extension()
            .and_then(|value| value.to_str())
        {
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => JS_FIRST,
            _ => TS_FIRST,
        }
    }
}
