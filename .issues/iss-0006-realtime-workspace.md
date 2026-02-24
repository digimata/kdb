---
id: 6
title: Realtime Multiplayer Workspace
status: proposed
priority: high
labels:
  - roadmap
  - spec
---

# ISS-0006 :: Realtime Multiplayer Workspace

Build a realtime-first collaborative workspace for knowledge bases, with shared state for both humans and agents.

- Multiple collaborators can edit shared context with low-latency updates.
- Agent sandbox edits can flow into shared state in near-realtime.
- Conflict handling and change visibility are clear and trustworthy.

## Problem

Current workflows are too slow for collaborative context work:

- Git-style commit/PR loops are excellent for source control, but too latent for realtime co-editing.
- Fast-moving teams and agents need shared context that updates instantly.
- Merge conflicts and delayed sync reduce trust in the current shared state.

## Desired Experience

- Multiple humans and agents can work in the same context repository at once.
- Everyone sees near-instant updates to notes, links, and structure.
- Shared views (bases/tables/graphs/indexes) reflect current state continuously.
- Collaboration feels like multiplayer docs with the rigor of a repository.

## Agent Sandbox Requirement

Agent workflows should treat sandboxed execution as first-class:

- An agent may run in an isolated sandbox/worktree while still attached to shared context.
- Agent edits should stream back to shared state in near-realtime (subject to policy/review).
- Sandboxes should continuously receive upstream changes to avoid stale context drift.
- Conflicts should be surfaced early as collaboration events.
- Humans should be able to inspect what the agent changed and why in timeline form.

## Principles

- Realtime-first collaboration.
- Local-first ergonomics with strong shared-state guarantees.
- Fast conflict handling with minimal user friction.
- Observable, trustworthy state.

## Relation to KDB

- `kdb` provides local authoring, indexing, and editor workflows.
- Shared `kdb` instances can power common, live collaborative views.
- Realtime workspace builds on this foundation rather than replacing it.

## Not Locked Yet

- No protocol or transport choice yet.
- No CRDT/OT strategy decision yet.
- No final merge/conflict model yet.
- No auth/permission topology yet.
