---
id: 54
title: "Self-update command"
status: proposed
priority: medium
labels:
  - feat
---

# ISS-0054 :: Self-update command

## Intent

`kdb update` checks for a newer release on GitHub and replaces the current binary in place. Same distribution pipeline as install.sh, but built into the binary.

## CLI

```
kdb update          # check + update if newer version available
kdb update --check  # just print whether an update is available
```

## Design

1. **Get current version** — `env!("CARGO_PKG_VERSION")` baked in at compile time
2. **Fetch latest tag** — `GET https://api.github.com/repos/dremnik/kdb/releases/latest`, parse `tag_name`
3. **Compare** — semver compare; if latest <= current, print "up to date" and exit
4. **Download** — detect platform (same logic as install.sh), download `kdb-{target}.tar.gz` + `checksums.txt` from the release
5. **Verify** — SHA-256 checksum match
6. **Replace** — extract binary to a tempfile, then rename over the current executable (`std::env::current_exe()`)
7. **Print** — show old version → new version

### Platform detection

Compile-time target triple via `env!("TARGET")` (set by build.rs or Cargo config). No runtime uname needed.

### Self-replacement

On Unix, replacing a running binary is fine — the OS holds the old inode until the process exits. Write to a tempfile in the same directory, then `std::fs::rename()` (atomic on same filesystem).

### Dependencies

- `ureq` — HTTP client (already considering for minimal footprint, or use `reqwest` if already in tree)
- `serde_json` — parse GitHub API response (may already be a dep)
- `flate2` + `tar` — extract the tarball

Check what's already in Cargo.toml before adding new deps.

## Changes

| File | Change |
|---|---|
| `src/main.rs` | Add `Update` subcommand variant with `--check` flag |
| `src/cmd.rs` | Add `update()` handler |
| `src/update.rs` (new) | Core update logic: fetch, compare, download, verify, replace |
| `Cargo.toml` | Add HTTP/archive deps if not already present |
| `build.rs` (maybe) | Set `TARGET` env var for platform detection |

## Verification

1. `kdb update --check` — prints current and latest version
2. `kdb update` on an older binary — downloads, verifies, replaces
3. `kdb update` when already latest — prints "up to date"
4. Checksum mismatch — errors cleanly, doesn't replace binary
5. No network — errors cleanly
