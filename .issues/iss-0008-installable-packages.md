---
id: 8
title: Installable KDB Packages
status: proposed
priority: medium
labels:
  - feat
---
> -------------------------------------------
> .issues/iss-0008-installable-packages.md
>
> ISS-0008 :: Installable KDB Packages    L19
>   • Intent                              L21
>   • Motivation                          L25
>   • Open Questions                      L31
> -------------------------------------------


# ISS-0008 :: Installable KDB Packages

## Intent

KDBs should be installable modules/packages, similar to npm or any other package manager. A user should be able to declare dependencies on external KDBs and pull them into their workspace.

## Motivation

- Knowledge bases are inherently composable — referencing and building on shared foundations is natural.
- There's no reason a KDB couldn't be published to and installed from npm (or a similar registry).
- This enables shared, versioned knowledge bases (e.g. a team's style guide, an API reference, a shared glossary) that can be depended on like any other package.

## Open Questions

- No dependency declaration exists today; if we add one, what should the format be and where should it live?
- Where do installed KDBs live on disk (e.g. a `kdb_modules/` directory)?
- Should the resolver/indexer treat installed KDBs as read-only?
- What registries to support? npm, a custom registry, git URLs, local paths?
- How does cross-KDB linking work (e.g. `[[dep-name::page]]` or similar)?
- Versioning and update strategy — lockfile?
