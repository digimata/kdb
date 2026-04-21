---
path: projects/kdb/benchmarks/agent-nav/t3-add-param/prompt.md
outline: |
  • T3 — Blast radius analysis        L13
    ◦ Per-repo prompts                L15
      ▪ mio                           L17
      ▪ poetry                        L20
      ▪ tokio                         L23
      ▪ airstore                      L26
      ▪ kubernetes                    L29
---

# T3 — Blast radius analysis

## Per-repo prompts

### mio
I want to add a `priority: u8` field to the `Token` struct. What's the blast radius?

### poetry
I want to add a `timeout` parameter to `RepositoryPool.__init__`. What's the blast radius?

### tokio
I want to add a `deadline: Option<Instant>` parameter to `AsyncRead::poll_read`. What's the blast radius?

### airstore
I want to add a `Priority int` field to the `Task` struct. What's the blast radius?

### kubernetes
I want to add a `Healthy() bool` method to the `Framework` interface. What's the blast radius?
