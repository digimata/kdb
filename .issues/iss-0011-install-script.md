---
id: 11
title: Install Script and Prebuilt Binaries
status: in_progress
priority: high
labels:
  - feat
  - release
---

# ISS-0011 :: Install Script and Prebuilt Binaries

Provide a one-liner install for people who don't have Rust/cargo:

```
curl -fsSL https://kdb.kernl.sh/install | bash
```

## Done

- [x] GitHub Release workflow — CI builds on tag push, 4 targets, checksums
- [x] Install script — platform detection, download, checksum verify, install to `~/.local/bin`
- [x] Binaries served from GitHub Releases

## Remaining: Host at kdb.kernl.sh

The install script and binary downloads need to be served from `kdb.kernl.sh`:

- **`kdb.kernl.sh/install`** — serve `install.sh` so `curl -fsSL https://kdb.kernl.sh/install | bash` works
- **Binary downloads** — either proxy to GitHub Releases or host directly
- **Version endpoint** — `kdb.kernl.sh/latest` or similar, returns latest version tag (needed by iss-0054 self-update)
- **Update install.sh** — point download URLs at `kdb.kernl.sh` instead of GitHub

### Open questions

- What's hosting the `kernl.sh` domain? (Cloudflare, Vercel, S3+CloudFront, etc.)
- Proxy to GitHub releases vs mirror the binaries?
