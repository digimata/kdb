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

- [x] `kdb.kernl.sh/install` — Vercel rewrite serves `public/install.sh`
- [x] `kdb.kernl.sh/latest` — Route Handler proxies GitHub Releases API (5-min cache)
- [x] `install.sh` updated to fetch version from `kdb.kernl.sh/latest`, binaries still from GitHub Releases
- [ ] DNS: point `kdb.kernl.sh` CNAME → `cname.vercel-dns.com` and add custom domain in Vercel
