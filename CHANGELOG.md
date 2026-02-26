# Changelog

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
