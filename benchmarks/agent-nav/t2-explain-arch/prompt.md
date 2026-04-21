---
path: projects/kdb/benchmarks/agent-nav/t2-explain-arch/prompt.md
outline: |
  • T2 — Explain module architecture        L13
    ◦ Per-repo prompts                      L15
      ▪ mio                                 L17
      ▪ poetry                              L20
      ▪ tokio                               L23
      ▪ airstore                            L26
      ▪ kubernetes                          L29
---

# T2 — Explain module architecture

## Per-repo prompts

### mio
Explain the architecture of `src/sys/` — what platform abstraction pattern does it use?

### poetry
Explain the architecture of `src/poetry/installation/` — what's the pipeline?

### tokio
Explain the architecture of `tokio/src/runtime/task/` — how do tasks work?

### airstore
Explain the architecture of `pkg/worker/` — how does task execution work?

### kubernetes
Explain the architecture of `pkg/scheduler/` — how does scheduling work?
