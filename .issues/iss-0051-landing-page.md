---
id: 51
title: "Landing page"
status: done
priority: medium
labels:
  - feat
---

# ISS-0051 :: Landing page

## Goal

Create a landing page for kdb at `apps/landing/`. Static site living in the repo alongside the Rust crate. The page is **agent-first** — kdb is a tool that AI coding agents install and use, so the primary audience is developers setting up agents.

## Structure

Next.js app at `apps/landing/`, deployed to Vercel.

```
apps/
  landing/
    app/
      page.tsx
      layout.tsx
      globals.css
    public/
      assets/
    package.json
    next.config.ts
    vercel.json
```

## Content

Reference: https://sonner.emilkowal.ski/ — fetch the actual HTML source to match the layout/structure.

Sections (mapping from sonner):
1. **Hero** — "kdb" + headline: "The fastest way for agents to navigate code + knowledge bases". Sub-copy: "Built with Rust. Your agents explore code ~[_]x faster and burn ~[_] less tokens. Fast, precise, no overhead." Then immediately: **Install button** — click copies to clipboard a prompt the user pastes into Claude (or any agent). The prompt tells the agent to install kdb and add the system prompt instructions. Something like: "Install kdb (`curl -fsSL https://kdb.kernl.sh/install | bash`) and add the following to your CLAUDE.md: ..." followed by the navigation instructions block below. GitHub link next to it.
2. **Demo video** — Side-by-side screen recording showing agent codebase exploration with vs without kdb (speed + token comparison). Reference style: https://x.com/SuhailKakar/status/2026305257257775524 — but split-screen showing the difference. Placeholder `<video>` element until we record it.
3. **Agent setup details** — Expanded view of what the install prompt contains. The prompt instructs the agent to put the following in its system prompt:

```
## Navigation

Prefer `kdb` over Glob/Grep for navigating projects. All commands work on both markdown and code files. Run `kdb init` at the project root if no `.kdb` directory exists.

| Task | Use |
|---|---|
| List symbols in a file (headings, functions, types, etc.) | `kdb symbols <file>` |
| Get specific symbol(s) | `kdb symbols <file> -s <name>...` |
| Find who imports a code symbol | `kdb refs <file> -s <symbol>` |
| Find inbound links to a markdown file/heading | `kdb refs <target>` |
| List outbound deps (links from md, imports from code) | `kdb deps <file>` |
| Explore project/directory structure | `kdb tree [path] [-L <depth>]` |
| Find broken links / orphans | `kdb check` |

Fall back to Grep/Glob when: searching for arbitrary strings/patterns, or kdb doesn't cover the query (e.g. regex search across file contents). Note: `kdb refs` is still maturing — if results look incomplete, verify with Grep.

`kdb` is our markdown knowledge base CLI + LSP. Source lives at `kdb/` in the monorepo (Rust, Cargo).

kdb
  init                Initialize a kdb project in a directory
  check [PATH]        Report broken links and orphan files
  tree [PATH]         Print a filtered directory tree
                      [-L <depth>] [-a] [-d] [-f] [-I <glob>] [-P <glob>] [-J]
  symbols <PATH>      Print symbols for a markdown or code file
                      [-s <name>...] [--json] [--public]
  refs <TARGET>       Find inbound references to a markdown target or code symbol
                      [-s <symbol>] [-c <N>] [--json] [--count]
  deps <TARGET>       Print direct dependencies for a file/symbol target
                      [--json]
  graph [PATH]        Render a dependency graph
  fmt [PATH]          Generate or update code index headers
  lsp                 Run the language server over stdio
```

4. **Usage** — terminal snippet showing `kdb symbols`, `kdb refs -s`
5. **Feature sections** — symbols, refs, deps, tree, check (with terminal output examples instead of interactive demos)
6. **Footer** — author credit, GitHub link

## Domain

`kdb.kernl.sh` — subdomain of `kernl.sh` (already on Vercel nameservers). Add via `vercel domains add kdb.kernl.sh <project>` once deployed.

## Notes

- Next.js app — `pnpm dlx create-next-app@latest apps/landing` to bootstrap, then build the single page
- Deploy to Vercel, add `kdb.kernl.sh` domain
- Add `.kdb/ignore` entry for `apps/` so kdb doesn't try to index it
- Primary audience: developers setting up AI coding agents — the install flow should make it dead simple to get kdb + system prompt instructions into an agent's context
