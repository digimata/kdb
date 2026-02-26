---
id: 11
title: Install Script and Prebuilt Binaries
status: done
priority: high
labels:
  - feat
  - release
---

# ISS-0011 :: Install Script and Prebuilt Binaries

Provide a one-liner install for people who don't have Rust/cargo:

```
curl -fsSL https://kdb.sh/install | bash
```

(or from GitHub raw URL until we have a domain)

## Requirements

### GitHub Release Workflow

- CI builds binaries on tag push (e.g. `v0.1.0`)
- Targets: macOS arm64, macOS x86_64, Linux x86_64, Linux arm64
- Uploads binaries as release assets

### Install Script

- Detects OS and architecture
- Downloads the correct binary from the latest GitHub release
- Places it in a sensible location (`~/.local/bin`, `/usr/local/bin`, or similar)
- Verifies checksum

### README

- Update install section with the curl one-liner as primary method
- Keep `cargo install --path .` as from-source fallback
