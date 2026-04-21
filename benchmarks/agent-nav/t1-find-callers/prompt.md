---
path: projects/kdb/benchmarks/agent-nav/t1-find-callers/prompt.md
outline: |
  • T1 — Find all callers        L14
    ◦ Per-repo prompts           L16
      ▪ mio                      L18
      ▪ poetry                   L21
      ▪ tokio                    L24
      ▪ airstore                 L27
      ▪ kubernetes               L30
    ◦ Ground truth               L33
---

# T1 — Find all callers

## Per-repo prompts

### mio
Find all files that use the `Token` struct from `src/token.rs`

### poetry
Find all files that use the `RepositoryPool` class from `src/poetry/repositories/repository_pool.py`

### tokio
Find all files that use the `AsyncRead` trait from `tokio/src/io/async_read.rs`

### airstore
Find all files that use the `Task` struct from `pkg/types/task.go`

### kubernetes
Find all files that use the `Framework` interface from `pkg/scheduler/framework/interface.go`

## Ground truth

| Repo | Symbol | Known callers (files) |
|---|---|---|
| mio | `Token` | 24 |
| poetry | `RepositoryPool` | 31 |
| tokio | `AsyncRead` | 57 |
| airstore | `Task` | 14 |
| kubernetes | `Framework` | 12 |
