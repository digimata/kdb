# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.11.0] — 2026-02-26

### Added

- `kdb symbols` now accepts multiple paths and directories (iss-0042, iss-0044)
  - directories are walked recursively, aggregating symbols from all supported files
  - multi-file output shows `── path` headers between each file's symbols
  - files are deduplicated when listed both explicitly and inside a directory
- `-s/--symbol` restricted to single definition file targets

## [0.10.2] — 2026-02-26

### Changed

- replace per-binding `scan_qualified_symbols` with single-pass `scan_all_qualified_symbols` that walks each file's tree once for all bindings (iss-0043)
- cache qualified access patterns across resolution loop iterations to avoid redundant tree walks (iss-0043)
- parallelize `load_code_files` with rayon — file I/O + tree-sitter parsing now concurrent (iss-0043)

### Performance

Benchmarks (warm, wall time):

| Repo | Files | Before | After | Speedup |
|---|---|---|---|---|
| mio (Rust) | ~200 | 0.28s | 0.05s | 5.6x |
| poetry (Python) | ~400 | 1.55s | 0.26s | 6x |
| ripgrep (Rust) | ~300 | 4.00s | 0.17s | 24x |
| tokio (Rust) | ~766 | 5.64s | 0.32s | 18x |
| airstore (Go) | ~312 | 13.05s | 0.16s | 82x |
| kubernetes (Go) | ~16k | ∞ (killed) | 9.0s | ∞ |

## [0.10.1] — 2026-02-25

### Changed

- parse tree-sitter tree once per file and reuse across symbol extraction, reexport collection, and usage scanning (iss-0043)
- parallelize per-file indexer phases (extract_symbols, build_reexport_lookup, link_usage_refs, link_go_same_package_refs) with rayon (iss-0043)

## [0.10.0] — 2026-02-25

### Changed

- replace single-pass reference resolution with fixed-point resolution loop (iss-0041)
- split `code.rs` into `code.rs`, `scope.rs`, and `scanner.rs` for maintainability

### Added

- `ModuleScope` per-file binding map enriched via `resolve_reexports()`, `expand_qualified_access()`, and `propagate_glob_imports()` before usage scanning
- compound patterns (grouped import → module → re-export) now resolve correctly

## [0.9.0] — 2026-02-25

### Added

- `symbols -s` now accepts multiple selectors (`-s Builder new find_paths`) (iss-0043)
- `symbols -s` text output includes a line number gutter with actual file line numbers (iss-0043)
- `symbols -s` body output includes preceding doc comments and attributes (iss-0043)

## [0.8.6] — 2026-02-25

### Added

- scan Go same-package files for symbol references without import statements (iss-0039.9)

## [0.8.5] — 2026-02-25

### Fixed

- resolve namespace/module qualified access (`bar.foo()`, `target.foo()`) for TS/JS and Python by detecting member expressions in the usage scanner (iss-0039.6, T4, P2)
- resolve Go dot imports (`import . "pkg"; Foo()`) by expanding namespace import symbols into the binding table (iss-0039.6, G3)
- follow one-hop re-exports through intermediary files (iss-0039.5)

## [0.8.4] — 2026-02-25

### Fixed

- recognize Rust type identifiers in parameter positions as usages, not declarations (iss-0039.3, R9)
- recognize JSX component identifiers (`jsx_identifier`) as usages and exclude JSX element tags from declaration detection (iss-0039.3, T11)

## [0.8.3] — 2026-02-25

### Fixed

- map Go package-qualified symbol usage to refs by separating import binding resolution (`pkg`) from symbol lookup (`Foo`) in the usage scanner and linker (iss-0039.2)

## [0.8.2] — 2026-02-25

### Fixed

- resolve aliased imports across all languages: introduce `ImportNames` struct carrying local→definition alias map, thread through all four resolvers and usage scanner (iss-0039.4)

### Changed

- remove `#[ignore]` from all refs eval tests — gap tests now fail instead of being silently skipped

## [0.8.1] — 2026-02-25

### Fixed

- fix Python `from X import name` symbol binding: fall back to parent module path when submodule resolution fails (iss-0039.1)

## [Unreleased]

### Changed

- update `CODEMAP.md` to reflect adopted project structure; close iss-0046
- split vault indexing from code indexing: extract `CodeIndex` and `ProjectIndex { vault, code }` from `VaultIndex`; vault-only commands no longer scan code files (iss-0048)
- extract `CmdContext` struct in `cmd.rs` wrapping `ProjectContext` with CLI conveniences (`from_path`, `build_index`, `rel_path`); refactor 7 commands to use it; rename `fn fmt()` → `fn format()` to resolve `crate::fmt` collision
- split `symbols/` into `extract/`, `tree.rs`, and `display.rs` per CC-3.6; merge render into display
- introduce `src/project/` module with `ProjectContext`, consolidating root discovery, config loading, ignore handling, path normalization, and file discovery
- centralize CodeLanguage for cross-module dispatch
- introduce extractor context for symbol extraction
- localize workspace caches by language
- move tsjs workspace logic and parser into resolver struct
- add LanguageResolver trait, restructure Go/Rust/Python resolvers
- close iss-0049 (replace code-parsing regexes with tree-sitter)

## [0.8.0] — 2026-02-25

### Added

- add `kdb refs <file> -s <symbol>` with import-resolved symbol reference indexing and declaration-site inclusion
- add `kdb refs -s <symbol> -c <N>` text context rendering with highlighted match lines and per-reference blocks

### Changed

- keep `refs` mode parity for `--count` and `--json` in symbol mode while scoping `--context` to `--symbol`

## [0.7.1] — 2026-02-25

### Added

- add Python package discovery with src/ layout and case-sensitive resolution

### Changed

- update issue tracker, add changelog tooling, refresh docs
- bump version to 0.7.1

### Docs

- regenerate changelog for v0.7.1

## [0.7.0] — 2026-02-24

### Other

- replace markdown parsing with tree-sitter
- add Rust workspace deps, Go go.work resolution, md symbol bodies, check scoping
- archive done issues, add labels to issue index

## [0.6.1] — 2026-02-24

### Other

- add vault root docs, update README, and refresh issue tracker
- use ignore crate for gitignore-aware parallel file walking

## [0.6.0] — 2026-02-24

### Other

- remove playground directory
- add issues and update readme
- add opencode.json to gitignore, update issues
- add demo gif, update readme, and add new issues
- update demo gif with higher quality encode
- vscode placeholder
- remove opencode.json from tracking
- cache vault index in lsp and add configurable ignore patterns
- watch markdown file changes to keep LSP index fresh
- prune completed issue 0010 tracking files
- update check output to gate orphan listings and treat orphans as warnings
- modularize cli and add symbols/refs plus index formatting
- use filepath index headers and standardize issue titles
- add kdb tree command with tree-style flags
- use language-native symbol display across symbols and fmt
- add import resolution, code deps, symbol bodies, and LSP formatting
- register kdb LSP for code languages in Zed extension

## [0.1.0] — 2026-02-18

### Other

- bootstrap kdb v0.1 — CLI, LSP, and zed extension
