---
path: projects/kdb/benchmarks/agent-nav/benchmark-design.md
outline: |
  • Agent Navigation Benchmark — Real Task Design                          L26
    ◦ Philosophy                                                           L28
    ◦ Task categories                                                      L34
    ◦ Task selection criteria                                              L53
    ◦ Task index                                                           L62
    ◦ Curated tasks                                                        L77
      ▪ B1 — Cross-cutting trait implementation (mio)                      L79
      ▪ B2 — Platform-consistent behavior change (mio)                     L98
      ▪ B3 — Bug fix with targeted test (mio)                             L117
      ▪ B4 — Add field to repository layer (poetry)                       L136
      ▪ B5 — Bug fix in locker (poetry)                                   L155
      ▪ B6 — Make private API public (tokio)                              L173
      ▪ B7 — Reduce lock contention (tokio)                               L192
      ▪ B8 — Feature gate + controller change (kubernetes)                L211
      ▪ B9 — TLS validation + connection invalidation (kubernetes)        L231
      ▪ B10 — Add method to network type (tokio)                          L250
    ◦ Evaluation                                                          L267
      ▪ Ground truth                                                      L269
      ▪ Metrics                                                           L277
      ▪ Execution                                                         L289
---

# Agent Navigation Benchmark — Real Task Design

## Philosophy

Synthetic microbenchmarks (T1-T3) measured isolated navigation primitives. Real developer tasks are end-to-end: the agent must orient, locate, understand, discover, change, and verify. Navigation is overhead — the less of it, the better.

The right metric isn't "how fast can you find callers" — it's "how efficiently does the agent complete a real task, measured in tokens consumed and correctness of the output."

## Task categories

Real tasks decompose into a navigation pipeline:

```
ORIENT → LOCATE → UNDERSTAND → DISCOVER → CHANGE → VERIFY
```

| Phase | What | kdb today | Potential |
|---|---|---|---|
| ORIENT | "Where in the repo?" | `tree` — good at scale | Same |
| LOCATE | "Which file/function?" | `symbols` — skip to right section | Same |
| UNDERSTAND | "What does this code do?" | Targeted reads via line numbers | Could summarize |
| DISCOVER | "What else needs changing?" | `refs` — import edges only | All usage edges |
| CHANGE | Make edits | N/A | N/A |
| VERIFY | Check correctness | N/A | N/A |

kdb's value = how much of ORIENT→DISCOVER it collapses vs grep+read.

## Task selection criteria

Good benchmark tasks:
- Come from real merged PRs (ground truth = the actual diff)
- Touch 2-10 files (not trivial, not massive)
- Have a clear description (agent can work from PR title + description)
- Require meaningful navigation (not just "edit this one file")
- Span different navigation patterns

## Task index

| ID | Repo | Commit | Task | Files | Nav pattern | kdb edge |
|---|---|---|---|---|---|---|
| B1 | mio | `d8d68ac` | Cross-cutting trait implementation | 7 | Find all trait implementors | Medium |
| B2 | mio | `8f7b87c` | Platform-consistent behavior change | 6 | Find call sites across platform modules | Low |
| B3 | mio | `8e5a4b5` | Targeted bug fix + regression test | 3 | Find one function, fix it | Low |
| B4 | poetry | `ab293dc5` | Add field across repository layer | 8 | Find all repository impls + tests | Strong |
| B5 | poetry | `df570ee7` | Fix non-deterministic lock ordering | 3 | Find one class, one function | Low |
| B6 | tokio | `d65165f7` | Make private API public | 4 | Find private function + all callers | Strong |
| B7 | tokio | `7a2135f4` | Reduce lock contention in uring driver | 5 | Trace lock usage through call chain | Medium |
| B8 | kubernetes | `f18f0df7feb` | Feature gate + controller change | 6 | Orient in huge repo, follow patterns | Strong |
| B9 | kubernetes | `39560700da1` | TLS validation + connection invalidation | 3 | Find code in staging/ subdirectory | Strong |
| B10 | tokio | `f1cb007a` | Add method to network type | 6 | Find type, follow method patterns | Medium |

## Curated tasks

### B1 — Cross-cutting trait implementation (mio)

**PR:** `d8d68ac` — "Implement more I/O safety traits"
**Repo:** mio (Rust, ~400 files)
**Scope:** 7 files, +224/-13
**Prompt:** "Implement `From<OwnedFd>` and `Into<OwnedFd>` for all types that implement `FromRawFd` and `IntoRawFd`. On Windows, implement `From<OwnedSocket>`, `Into<OwnedSocket>`, and `AsSocket` for types that implement `FromRawSocket`, `IntoRawSocket`, and `AsRawSocket`."

**Nav challenge:** Find all types that implement FromRawFd/IntoRawFd across the codebase, then add parallel impls. This is a DISCOVER-heavy task — the agent must find all implementors of a trait.

**Information floor:**
- ORIENT: repo structure (1 call)
- DISCOVER: all types implementing FromRawFd/IntoRawFd (~7 types across 7 files)
- UNDERSTAND: read existing impl pattern in one file (~30 lines)
- CHANGE: add matching impls to all 7 files

**kdb advantage:** `refs` could find all files referencing FromRawFd. But finding *implementors* specifically requires grep `impl.*FromRawFd.*for`.

---

### B2 — Platform-consistent behavior change (mio)

**PR:** `8f7b87c` — "Use same backlog argument as std in listen calls"
**Repo:** mio (Rust, ~400 files)
**Scope:** 6 files, +75/-10
**Prompt:** "The listen() calls use hardcoded backlog values. Change them to use the same backlog argument as std, with proper platform-dependent defaults."

**Nav challenge:** Find all listen() call sites across platform-specific modules, understand std's approach, apply consistently.

**Information floor:**
- ORIENT: platform module structure (1 call)
- DISCOVER: all listen() calls (grep, ~4 sites)
- UNDERSTAND: how std handles backlog (read std source or docs)
- CHANGE: add backlog constant to sys/mod.rs, update all call sites

**kdb advantage:** `tree` helps orient in platform modules. But listen() is a function call pattern — needs grep.

---

### B3 — Bug fix with targeted test (mio)

**PR:** `8e5a4b5` — "Fix peek reregister after would block"
**Repo:** mio (Rust, ~400 files)
**Scope:** 3 files, +131/-1
**Prompt:** "Fix the bug where peek doesn't properly reregister after a would-block error on TCP streams."

**Nav challenge:** Find the peek implementation, understand the registration lifecycle, fix the reregister logic, write a regression test.

**Information floor:**
- LOCATE: find peek in tcp/stream.rs (1 call)
- UNDERSTAND: read peek + reregister logic (~50 lines)
- DISCOVER: find test file and test patterns (~1 call)
- CHANGE: fix 1 line, write regression test

**kdb advantage:** `symbols` finds peek quickly. Small focused task — baseline is fine too.

---

### B4 — Add field to repository layer (poetry)

**PR:** `ab293dc5` — "Add size and upload-time to file info if available"
**Repo:** poetry (Python, ~1K files)
**Scope:** 8 files, +107/-20
**Prompt:** "Add size and upload-time fields to file info in the repository layer. The http_repository, json link source, and pypi_repository should all expose these fields when available."

**Nav challenge:** Find all repository implementations, understand the file info data flow, add the fields consistently, update tests.

**Information floor:**
- ORIENT: repository directory structure (1 call)
- DISCOVER: all repository classes that handle file info (3 source files + 4 test files)
- UNDERSTAND: read file info handling in one repo (~30 lines)
- CHANGE: add fields to 3 source files, update 4 test files

**kdb advantage:** `refs` to find all repository implementations. `deps` to trace the file info data flow. Strong potential if agent uses both.

---

### B5 — Bug fix in locker (poetry)

**PR:** `df570ee7` — "Fix non-deterministic dependency constraint ordering in lock file"
**Repo:** poetry (Python, ~1K files)
**Scope:** 3 files, +69/-5
**Prompt:** "The lock file has non-deterministic ordering of dependency constraints, causing unnecessary diffs. Fix the ordering to be deterministic."

**Nav challenge:** Find the locker serialization code, identify where constraints are written without sorting, add sorting, write test.

**Information floor:**
- LOCATE: find Locker class and constraint serialization (1-2 calls)
- UNDERSTAND: read the serialization function (~20 lines)
- CHANGE: add sort(), write test

**kdb advantage:** `symbols` finds the Locker class. Small focused change.

---

### B6 — Make private API public (tokio)

**PR:** `d65165f7` — "Make `is_rt_shutdown_err` method public"
**Repo:** tokio (Rust, ~9.7K files)
**Scope:** 4 files, +153/-42
**Prompt:** "Make the `is_rt_shutdown_err` helper method public so external users can check if an error is due to runtime shutdown. Move it from the internal process module to the public runtime module."

**Nav challenge:** Find the existing private helper, understand its usage, move it to public API, update all call sites, add tests.

**Information floor:**
- LOCATE: find is_rt_shutdown_err (1 call)
- DISCOVER: find all current callers (kdb refs or grep, ~2 files)
- UNDERSTAND: read the function + its callers (~40 lines)
- CHANGE: move function, update imports, add public tests

**kdb advantage:** `refs` finds callers precisely. `symbols` locates in large file. Medium-sized repo where navigation overhead matters.

---

### B7 — Reduce lock contention (tokio)

**PR:** `7a2135f4` — "Avoid lock acquisition after uring init"
**Repo:** tokio (Rust, ~9.7K files)
**Scope:** 5 files, +46/-74
**Prompt:** "The io_uring driver acquires a lock on every operation after initialization. Refactor to avoid the lock acquisition once uring is fully initialized."

**Nav challenge:** Find the uring driver, understand the locking pattern, trace through the fs operations that acquire the lock, refactor.

**Information floor:**
- LOCATE: find uring driver (1 call)
- UNDERSTAND: read lock pattern in driver + fs operations (~100 lines)
- DISCOVER: find all fs operations that acquire the lock (3 files)
- CHANGE: refactor lock out of hot path

**kdb advantage:** `deps` traces from driver to fs operations. `symbols` navigates large files. Good DISCOVER task.

---

### B8 — Feature gate + controller change (kubernetes)

**PR:** `f18f0df7feb` — "Add read-own-writes to replicaset controller"
**Repo:** kubernetes (Go, ~28K files)
**Scope:** 6 files, +119/-12
**Prompt:** "Add the ability for the replicaset controller to read its own writes. Add a feature gate and metrics."

**Nav challenge:** Find the replicaset controller in a 28K-file repo, understand its write path, follow existing patterns for feature gates and metrics.

**Information floor:**
- ORIENT: find replicaset controller in huge repo (1 call)
- LOCATE: find the write path (1-2 calls)
- DISCOVER: find feature gate patterns to follow (1-2 calls), find metrics patterns (1 call), find doc templates (1 call)
- UNDERSTAND: read controller logic + existing feature gate example (~100 lines)
- CHANGE: edit 6 files following patterns

**kdb advantage:** `tree` + `symbols` critical for orientation in 28K files. `refs` finds feature gate usage patterns. Biggest potential win.

---

### B9 — TLS validation + connection invalidation (kubernetes)

**PR:** `39560700da1` — "Enforce TLS validation and invalidate bad connections"
**Repo:** kubernetes (Go, ~28K files)
**Scope:** 3 files, +101/-23
**Prompt:** "The aggregated API server proxy doesn't enforce TLS validation. Fix it to configure CAData from the apiService spec, and invalidate cached connections when availability checks fail."

**Nav challenge:** Find the remote availability controller in the staging directory of a huge repo, understand the HTTP transport setup, add TLS config.

**Information floor:**
- ORIENT: find the kube-aggregator code (1 call — hard without tree/grep)
- LOCATE: find the availability controller (1 call)
- UNDERSTAND: read transport setup + availability check (~80 lines)
- CHANGE: modify 2 source files, add test

**kdb advantage:** `tree` critical for finding the right directory. Without it, agent greps blindly in 28K files.

---

### B10 — Add method to network type (tokio)

**PR:** `f1cb007a` — "Add TcpStream::set_zero_linger"
**Repo:** tokio (Rust, ~9.7K files)
**Scope:** 6 files, +72/-10
**Prompt:** "Add a `set_zero_linger` method to TcpStream and TcpSocket. This sets SO_LINGER with zero timeout, useful for abortive connection close."

**Nav challenge:** Find TcpStream and TcpSocket, understand existing set_* method patterns, add new method following pattern, update tests.

**Information floor:**
- LOCATE: find TcpStream + TcpSocket (1-2 calls)
- DISCOVER: find existing set_* methods as pattern to follow (1 call)
- UNDERSTAND: read one existing set_* impl (~10 lines)
- CHANGE: add to 2 source files, update 4 test files

**kdb advantage:** `symbols` finds methods in TcpStream. `refs` could find test files. Pattern-following task.

## Evaluation

### Ground truth

For each task, the ground truth is the actual PR diff. We evaluate:

1. **File recall:** what % of files from the real diff did the agent touch?
2. **File precision:** what % of files the agent touched were in the real diff?
3. **Change correctness:** does the diff produce equivalent behavior? (manual review)

### Metrics

| Metric | How |
|---|---|
| Tool calls | Count from JSONL |
| Input tokens | Max input tokens across turns |
| Output tokens | Total output tokens |
| Cost | From API usage |
| Wall time | Clock time |
| File recall | Files touched ∩ ground truth / ground truth |
| File precision | Files touched ∩ ground truth / files touched |

### Execution

1. Check out repo at parent commit of the PR
2. Give agent the prompt + "make the changes, don't commit"
3. Capture full JSONL trace
4. Diff agent's changes against ground truth
5. Score
