---
path: projects/kdb/benchmarks/agent-nav/t1-find-callers/results.md
outline: |
  • t1-find-callers      L12
    ◦ airstore           L23
    ◦ kubernetes         L37
    ◦ mio                L64
    ◦ poetry             L73
    ◦ tokio              L81
---

# t1-find-callers

| Repo | kdb wall | kdb turns | kdb calls | kdb cost | base wall | base turns | base calls | base cost | speedup |
|---|---|---|---|---|---|---|---|---|---|
| airstore | 12s | 2 | 1 | $0.05 | 29s | 8 | 7 | $0.12 | 2.4x |
| kubernetes | 15s | 3 | 2 | $0.07 | 86s | 20 | 19 | $0.33 | 5.7x |
| mio | 12s | 2 | 1 | $0.06 | 19s | 3 | 2 | $0.06 | 1.6x |
| poetry | 14s | 2 | 1 | $0.06 | 13s | 2 | 1 | $0.04 | 0.9x |
| tokio | 23s | 2 | 1 | $0.08 | 46s | 3 | 2 | $0.13 | 2.0x |
| **avg** | 15s | 2.2 | 1.2 | $0.06 | 39s | 7.2 | 6.2 | $0.14 | 2.5x |

## airstore

**kdb** — 12s, 2 turns, $0.05
  1. `Bash: kdb refs pkg/types/task.go -s Task --files`

**baseline** — 29s, 8 turns, $0.12
  1. `Read: /Users/andjones/Documents/repos/airstore/pkg/types/task.go`
  2. `Grep: pattern='types\\.Task[^A-Za-z_]' path=/Users/andjones/Documents/repos/airstore mode=files_with_matches`
  3. `Grep: pattern='\\*types\\.Task[^A-Za-z_]' path=/Users/andjones/Documents/repos/airstore mode=files_with_matches`
  4. `Grep: pattern='\\[]types\\.Task' path=/Users/andjones/Documents/repos/airstore mode=files_with_matches`
  5. `Grep: pattern='"\\. \\".*pkg/types\\""' path=/Users/andjones/Documents/repos/airstore mode=files_with_matches`
  6. `Grep: pattern='types\\.Task\\b' path=/Users/andjones/Documents/repos/airstore mode=files_with_matches`
  7. `Grep: pattern='\\bTask\\b' path=/Users/andjones/Documents/repos/airstore/pkg/types mode=files_with_matches`

## kubernetes

**kdb** — 15s, 3 turns, $0.07
  1. `Bash: kdb symbols pkg/scheduler/framework/interface.go -s Framework`
  2. `Bash: kdb refs pkg/scheduler/framework/interface.go -s Framework --files`

**baseline** — 86s, 20 turns, $0.33
  1. `Grep: pattern='type Framework interface' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/interface.go mode=content`
  2. `Grep: pattern='\\bframework\\.Framework\\b' path=. mode=files_with_matches`
  3. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework mode=files_with_matches`
  4. `Grep: pattern='k8s\\.io/kubernetes/pkg/scheduler/framework"' path=. mode=files_with_matches`
  5. `Grep: pattern='schedframework "k8s\\.io/kubernetes/pkg/scheduler/framework"' path=. mode=files_with_matches`
  6. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler mode=files_with_matches`
  7. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/kubelet mode=files_with_matches`
  8. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/test/integration mode=files_with_matches`
  9. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/kubelet/lifecycle/handlers.go mode=content`
  10. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/kubelet/kubelet.go mode=content`
  11. `Grep: pattern='framework\\.\\w*Framework' path=/Users/andjones/Documents/repos/kubernetes/test/integration mode=files_with_matches`
  12. `Grep: pattern='framework\\.Framework' path=/Users/andjones/Documents/repos/kubernetes/test/integration mode=files_with_matches`
  13. `Grep: pattern='framework\\.Framework' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler mode=files_with_matches`
  14. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/plugins/nodedeclaredfeatures/nodedeclaredfeatures.go mode=content`
  15. `Grep: pattern='scheduler/framework"' path=/Users/andjones/Documents/repos/kubernetes/test mode=content`
  16. `Grep: pattern='schedulerframework\\.Framework|schedframework\\.Framework' path=/Users/andjones/Documents/repos/kubernetes/test/integration mode=files_with_matches`
  17. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/test/integration/util/util.go mode=content`
  18. `Grep: pattern='\\bFramework\\b' path=/Users/andjones/Documents/repos/kubernetes/test/integration/scheduler_perf/scheduler_perf.go mode=content`
  19. `Grep: pattern='framework\\.Framework' path=/Users/andjones/Documents/repos/kubernetes/CHANGELOG mode=files_with_matches`

## mio

**kdb** — 12s, 2 turns, $0.06
  1. `Bash: kdb refs src/token.rs -s Token`

**baseline** — 19s, 3 turns, $0.06
  1. `Read: /Users/andjones/Documents/repos/mio/src/token.rs`
  2. `Grep: pattern='use.*Token|Token\\b' path=. mode=files_with_matches`

## poetry

**kdb** — 14s, 2 turns, $0.06
  1. `Bash: kdb refs src/poetry/repositories/repository_pool.py -s RepositoryPool --files`

**baseline** — 13s, 2 turns, $0.04
  1. `Grep: pattern='RepositoryPool' path=. mode=files_with_matches`

## tokio

**kdb** — 23s, 2 turns, $0.08
  1. `Bash: kdb refs tokio/src/io/async_read.rs -s AsyncRead --files`

**baseline** — 46s, 3 turns, $0.13
  1. `Grep: pattern='use.*AsyncRead' path=/Users/andjones/Documents/repos/tokio mode=files_with_matches`
  2. `Grep: pattern='AsyncRead' path=/Users/andjones/Documents/repos/tokio mode=files_with_matches`

