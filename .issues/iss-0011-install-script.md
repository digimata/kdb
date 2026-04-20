---
id: 11
title: Install Script and Prebuilt Binaries
status: in_progress
priority: high
labels:
  - feat
  - release
path: kdb/.issues/iss-0011-install-script.md
outline: |
  • ISS-0011 :: Install Script and Prebuilt Binaries      L16
    ◦ Done                                                L24
    ◦ Remaining: Host at kdb.digimata.dev                     L30
---

# ISS-0011 :: Install Script and Prebuilt Binaries

Provide a one-liner install for people who don't have Rust/cargo:

```
curl -fsSL https://kdb.digimata.dev/install | bash
```

## Done

- [x] GitHub Release workflow — CI builds on tag push, 4 targets, checksums
- [x] Install script — platform detection, download, checksum verify, install to `~/.local/bin`
- [x] Binaries served from GitHub Releases

## Remaining: Host at kdb.digimata.dev

- [x] `kdb.digimata.dev/install` — Vercel rewrite serves `public/install.sh`
- [x] `kdb.digimata.dev/latest` — Route Handler proxies GitHub Releases API (5-min cache)
- [x] `install.sh` updated to fetch version from `kdb.digimata.dev/latest`, binaries still from GitHub Releases
- [ ] DNS: point `kdb.digimata.dev` CNAME → `cname.vercel-dns.com` and add custom domain in Vercel
