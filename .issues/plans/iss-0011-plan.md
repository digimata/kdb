---
title: "Install Script and Prebuilt Binaries"
date: 2026-02-26
status: draft
affects: "release, CI, install"
---

## Context

kdb currently requires Rust/cargo to install (`cargo install --path .`). There's no CI pipeline, no prebuilt binaries, and no quick install path for users without a Rust toolchain. We need a GitHub Actions release workflow and an install script.

## Changes

### 1. GitHub Actions release workflow (`.github/workflows/release.yml`)

Triggers on tag push matching `v*`. Builds binaries for 4 targets using a matrix strategy with `cross` for Linux cross-compilation:

| Target | Runner |
|--------|--------|
| `aarch64-apple-darwin` | `macos-latest` |
| `x86_64-apple-darwin` | `macos-latest` |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` |
| `aarch64-unknown-linux-gnu` | `ubuntu-latest` (via cross) |

Each build:
- Compiles release binary
- Tars it: `kdb-<target>.tar.gz`
- Uploads as artifact

Final job:
- Collects all artifacts
- Generates `checksums.txt` (SHA256, one line per archive)
- Creates GitHub release with all `.tar.gz` + `checksums.txt` attached
- Uses tag name as release title, auto-generates release notes from commits

### 2. Install script (`install.sh`)

Shell script at repo root. Steps:

1. Detect OS (`uname -s`) and arch (`uname -m`), map to target triple
2. Fetch latest release tag from GitHub API (`/repos/dremnik/kdb/releases/latest`)
3. Download the correct `.tar.gz` and `checksums.txt`
4. Verify SHA256 checksum (`sha256sum` or `shasum -a 256`)
5. Extract binary to `~/.local/bin/kdb` (create dir if needed)
6. Print success message + PATH hint if `~/.local/bin` isn't on PATH

Supported platforms: macOS arm64/x86_64, Linux x86_64/arm64. Anything else → error with cargo fallback instructions.

### 3. README update

Replace the Quickstart install step with curl one-liner as primary, cargo as fallback:

```
curl -fsSL https://kernl.sh/kdb/install | bash
```

(Assumes `kernl.sh/kdb/install` serves the raw `install.sh` from the repo — hosting/redirect setup is separate from this issue.)

### 4. Issue bookkeeping

Update issue status to `done`, update index.

## Files touched

```
┌─────────────────────────────────┬────────────────────────────────────────┐
│              File               │                Action                  │
├─────────────────────────────────┼────────────────────────────────────────┤
│ .github/workflows/release.yml  │ Create — release CI workflow            │
│ install.sh                     │ Create — install script                 │
│ README.md                      │ Edit — update Quickstart install section│
│ .issues/iss-0011-*.md          │ Edit — status → done                   │
│ .issues/index.md               │ Edit — status → done                   │
└─────────────────────────────────┴────────────────────────────────────────┘
```

## Verification

- [ ] `shellcheck install.sh` passes clean
- [ ] Workflow YAML validates (`actionlint` or manual review)
- [ ] `kdb check` — no broken links
- [ ] Dry-run install script locally (will fail on download since no release exists yet, but logic paths can be reviewed)
- [ ] First real test: push a `v0.12.1` tag after merge and verify the full flow
