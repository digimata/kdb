---
id: 41
title: "Rust resolver: custom `[lib] path` support"
status: done
priority: medium
labels:
  - bug
  - deps
  - rust
---

# ISS-0041 :: Rust resolver: custom `[lib] path` support

## Bug

The Rust import resolver assumes crate entry points are `src/lib.rs`, `src/main.rs`, or `src/mod.rs`. Crates that declare a custom `[lib] path` in `Cargo.toml` (e.g. `path = "src/gpui.rs"`) fail to resolve both:

1. **`mod` declarations** — `resolve_mod_decl` checks if the source file is named `lib.rs`/`main.rs`/`mod.rs` to determine the module root directory. A custom entry like `src/agent.rs` is treated as a regular module, so `mod db` looks for `src/agent/db.rs` instead of `src/db.rs`.

2. **Cross-crate imports** — `rust_crate_entry_path` only checks `lib.rs`/`main.rs`/`mod.rs`. When another crate does `use agent::foo`, the resolver can't find the entry point to fall back on.

## Reproduction

Zed uses `[lib] path = "src/<crate_name>.rs"` across all crates:

```bash
cd ~/Documents/repos/zed
kdb deps crates/agent/src/agent.rs
# Returns only 3 deps instead of ~40+
# All `mod` declarations resolve to None
# Most cross-crate `use` statements resolve to None
```

Tokio (which uses standard `src/lib.rs`) works correctly.

## Root cause

- `resolve_mod_decl` (L411): hardcoded `lib.rs`/`main.rs`/`mod.rs` check
- `rust_crate_entry_path` (L585): only tries `lib.rs`/`main.rs`/`mod.rs`
- `RustWorkspaceCrate` stores `src_root` but not the actual lib entry file

## Fix

1. Parse `[lib] path` (and `[[bin]] path`) from each crate's `Cargo.toml` in `build_workspace_cache`
2. Store the entry point file path in `RustWorkspaceCrate`
3. Use it in `resolve_mod_decl` to correctly identify crate root files
4. Use it in `rust_crate_entry_path` as the fallback entry point

## Affected crates

Any Rust project using custom `[lib] path` — notably Zed, which does this for every crate.
