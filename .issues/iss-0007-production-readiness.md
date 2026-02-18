---
id: 7
title: Production Readiness
status: proposed
priority: high
labels:
  - roadmap
  - spec
  - quality
---

# 0007 :: Production Readiness

## Intent

Ship a simple end-to-end implementation first, then harden it in small, testable steps.

## Current Validation-First Implementation

- Root discovery is explicit via `.kdb/` and upward search.
- Indexing is full-rebuild (no incremental update logic).
- CLI scope is intentionally narrow: `kdb check`, `kdb outline`.
- LSP currently favors simplicity over perfect edge-case behavior:
  - Go-to-definition resolves links from on-disk content.
  - Completion supports basic file + heading contexts.
  - Document symbols provide heading tree outline.

## Hardening Roadmap

### Phase 1: Correctness Baseline

- Expand parser fixture coverage for headings, duplicate anchors, nested links, and wikilink aliases.
- Add explicit anchor-slug compatibility tests against expected markdown behavior.
- Add resolver tests for path normalization and root-boundary enforcement.
- Add integration tests for `kdb check` exit codes and machine-readable output format.

Done when:
- Parser and resolver fixture suite covers core edge cases.
- `kdb check` behavior is deterministic and regression-protected.

### Phase 2: LSP Robustness

- Use open document state as the source of truth when available (not only on-disk files).
- Add document diagnostics publishing on open/change/save.
- Add hover previews for linked section first paragraph.
- Add tests for position mapping and link-under-cursor behavior.

Done when:
- LSP behavior is stable for both unsaved and saved files.
- Broken links consistently appear as diagnostics.

### Phase 3: Performance and Index Strategy

- Introduce incremental index updates for changed files.
- Add debounce/coalescing for rapid edit streams.
- Add basic telemetry/log timings for parse/index/resolve phases.
- Profile medium/large vaults and optimize hotspots.

Done when:
- Interactive LSP operations remain responsive on representative vault sizes.

### Phase 4: Config and Policy

- Extend `.kdb/config.toml` with `ignore`, link style policy, and warning severity config.
- Validate config schema with clear errors and defaults.
- Add compatibility tests for no-config and config-present modes.

Done when:
- Config changes are explicit, validated, and backward-compatible.

### Phase 5: OSS Quality Bar

- Add CI matrix: `fmt`, `clippy`, `test`, and release build.
- Add CONTRIBUTING + architecture notes.
- Add versioned changelog and release process.
- Add golden fixtures for CLI output stability.

Done when:
- Contributors can run one command and get consistent verification locally and in CI.

## Non-Goals for Initial Hardening

- Rename symbol and find-all-references.
- Graph visualization commands.
- Markdown formatting (`kdb fmt`).

These remain optional future features after core correctness and LSP reliability are stable.
