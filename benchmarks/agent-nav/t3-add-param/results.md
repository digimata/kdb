---
path: projects/kdb/benchmarks/agent-nav/t3-add-param/results.md
outline: |
  • t3-add-param       L12
    ◦ airstore         L23
    ◦ kubernetes       L67
    ◦ mio             L101
    ◦ poetry          L121
    ◦ tokio           L137
---

# t3-add-param

| Repo | kdb wall | kdb turns | kdb calls | kdb cost | base wall | base turns | base calls | base cost | speedup |
|---|---|---|---|---|---|---|---|---|---|
| airstore | 66s | 19 | 18 | $0.32 | 53s | 21 | 20 | $0.21 | 0.8x |
| kubernetes | 80s | 17 | 16 | $0.34 | 59s | 13 | 12 | $0.28 | 0.7x |
| mio | 50s | 8 | 7 | $0.19 | 45s | 8 | 7 | $0.15 | 0.9x |
| poetry | 34s | 7 | 6 | $0.14 | 25s | 5 | 4 | $0.09 | 0.7x |
| tokio | 75s | 22 | 21 | $0.28 | 36s | 4 | 3 | $0.11 | 0.5x |
| **avg** | 61s | 14.6 | 13.6 | $0.25 | 44s | 10.2 | 9.2 | $0.17 | 0.7x |

## airstore

**kdb** — 66s, 19 turns, $0.32
  1. `Bash: kdb tree -L 2`
  2. `Grep: pattern='struct Task\\b' path=. mode=content`
  3. `Bash: kdb symbols pkg/types/task.go`
  4. `Bash: kdb refs pkg/types/task.go -s Task`
  5. `Read: /Users/andjones/Documents/repos/airstore/pkg/types/task.go`
  6. `Bash: kdb refs proto/gateway.pb.go -s Task --files`
  7. `Grep: pattern='proto/gateway.proto' path=. mode=files_with_matches`
  8. `Read: /Users/andjones/Documents/repos/airstore/proto/gateway.proto`
  9. `Grep: pattern='message Task \\{' path=/Users/andjones/Documents/repos/airstore/proto/gateway.proto mode=content`
  10. `Grep: pattern='TaskResponse' path=/Users/andjones/Documents/repos/airstore/pkg/api/v1/tasks.go mode=content`
  11. `Grep: pattern='CREATE TABLE.*task' path=. mode=files_with_matches`
  12. `Read: /Users/andjones/Documents/repos/airstore/pkg/repository/task_postgres.go`
  13. `Read: /Users/andjones/Documents/repos/airstore/pkg/repository/task_queue.go`
  14. `Read: /Users/andjones/Documents/repos/airstore/pkg/repository/task_postgres.go`
  15. `Grep: pattern='task' path=/Users/andjones/Documents/repos/airstore/pkg/repository/backend_postgres_migrations/001_initial.go mode=content`
  16. `Glob: pattern='pkg/repository/backend_postgres_migrations/*.go'`
  17. `Read: /Users/andjones/Documents/repos/airstore/pkg/filesystem/vnode/tasks.go`
  18. `Read: /Users/andjones/Documents/repos/airstore/pkg/gateway/services/gateway.go`

**baseline** — 53s, 21 turns, $0.21
  1. `Grep: pattern='type Task struct' path=. mode=content`
  2. `Grep: pattern='types\\.Task[^T]' path=. mode=files_with_matches`
  3. `Grep: pattern='\\bTask\\b' path=pkg/types/task.go mode=content`
  4. `Grep: pattern='\\.Task\\{' path=. mode=files_with_matches`
  5. `Grep: pattern='CreateTask|InsertTask|UpdateTask|SaveTask' path=. mode=files_with_matches`
  6. `Grep: pattern='task\\.(Priority|Status|Type|Prompt|Image|Entrypoint)' path=. mode=files_with_matches`
  7. `Grep: pattern='proto.*Task|Task.*proto' path=proto mode=files_with_matches`
  8. `Glob: pattern='proto/*.proto'`
  9. `Grep: pattern='message Task ' path=. mode=content`
  10. `Grep: pattern='message Task ' path=proto/gateway.proto mode=content`
  11. `Grep: pattern='message CreateTaskRequest' path=. mode=content`
  12. `Grep: pattern='TaskStore|TaskRepository|TaskRepo' path=. mode=files_with_matches`
  13. `Grep: pattern='task_postgres' path=. mode=files_with_matches`
  14. `Grep: pattern='INSERT INTO.*tasks|UPDATE.*tasks' path=. mode=files_with_matches`
  15. `Grep: pattern='migration.*task|task.*migration' path=pkg/repository/backend_postgres_migrations mode=files_with_matches`
  16. `Grep: pattern='tasks.*SDK|SDK.*task|task.*sdk' path=sdk mode=files_with_matches`
  17. `Grep: pattern='Task|task' path=pkg/scheduler mode=files_with_matches`
  18. `Grep: pattern='types\\.Task\\b' path=pkg/worker mode=content`
  19. `Grep: pattern='types\\.Task\\b' path=pkg/scheduler mode=content`
  20. `Grep: pattern='types\\.Task\\b' path=pkg/repository/task_queue.go mode=content`

## kubernetes

**kdb** — 80s, 17 turns, $0.34
  1. `Bash: kdb tree -L 2`
  2. `Grep: pattern='type Framework interface' path=. mode=files_with_matches`
  3. `Bash: kdb symbols pkg/scheduler/framework/interface.go -s Framework`
  4. `Bash: kdb refs pkg/scheduler/framework/interface.go -s Framework --files`
  5. `Bash: kdb symbols pkg/scheduler/framework/runtime/framework.go -s frameworkImpl`
  6. `Bash: kdb symbols pkg/scheduler/testing/framework/framework_helpers.go -s MockFramework`
  7. `Grep: pattern='Framework\\b' path=pkg/scheduler/framework/autoscaler_contract/framework_contract_test.go mode=content`
  8. `Bash: kdb symbols pkg/scheduler/testing/framework/framework_helpers.go`
  9. `Grep: pattern='Framework' path=pkg/scheduler/framework/autoscaler_contract/framework_contract_test.go mode=content`
  10. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/autoscaler_contract/framework_contract_test.go`
  11. `Grep: pattern='var.*framework\\.Framework|framework\\.Framework\\b' path=pkg/scheduler mode=content`
  12. `Grep: pattern='framework\\.Framework\\b' path=pkg/scheduler/schedule_one_test.go mode=content`
  13. `Grep: pattern='framework\\.Framework\\b' path=pkg/scheduler/framework/runtime/framework_test.go mode=content`
  14. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/schedule_one_test.go`
  15. `Grep: pattern='framework\\.Framework' path=test mode=files_with_matches`
  16. `Grep: pattern='framework\\.Framework' path=staging mode=files_with_matches`

**baseline** — 59s, 13 turns, $0.28
  1. `Grep: pattern='type Framework interface' path=. mode=content`
  2. `Grep: pattern='Framework interface' path=. mode=files_with_matches`
  3. `Grep: pattern='framework\\.Framework' path=. mode=files_with_matches`
  4. `Grep: pattern='implements Framework|Framework\\)|var .* Framework' path=. mode=files_with_matches`
  5. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/interface.go`
  6. `Grep: pattern='scheduler/framework\\.Framework|"k8s.io/kubernetes/pkg/scheduler/framework"' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler mode=files_with_matches`
  7. `Grep: pattern='framework\\.Framework' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler mode=files_with_matches`
  8. `Grep: pattern='var _ framework\\.Framework|_ framework\\.Framework =' path=. mode=content`
  9. `Grep: pattern='type frameworkImpl struct|MockFramework|fakeFramework|type.*Framework struct' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler mode=content`
  10. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/testing/framework/framework_helpers.go`
  11. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/autoscaler_contract/framework_contract_test.go`
  12. `Grep: pattern='map\\[string\\]framework\\.Framework|Framework\\b' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler/profile/profile.go mode=content`

## mio

**kdb** — 50s, 8 turns, $0.19
  1. `Bash: kdb tree -L 2`
  2. `Grep: pattern='struct Token' path=. mode=content`
  3. `Bash: kdb refs src/token.rs -s Token`
  4. `Read: /Users/andjones/Documents/repos/mio/src/token.rs`
  5. `Grep: pattern='Token\\(' path=. mode=files_with_matches`
  6. `Grep: pattern='\\.0\\b' path=/Users/andjones/Documents/repos/mio/src mode=content`
  7. `Grep: pattern='token\\.0' path=. mode=content`

**baseline** — 45s, 8 turns, $0.15
  1. `Grep: pattern='struct Token' path=. mode=content`
  2. `Grep: pattern='Token' path=. mode=files_with_matches`
  3. `Read: /Users/andjones/Documents/repos/mio/src/token.rs`
  4. `Grep: pattern='Token\\(' path=. mode=files_with_matches`
  5. `Grep: pattern='\\.0\\b' path=/Users/andjones/Documents/repos/mio/src/token.rs mode=content`
  6. `Grep: pattern='token\\.0|Token\\(\\d|Token\\([a-z_]' path=. mode=content`
  7. `Grep: pattern='token\\.0|Token\\)\\.0|\\.0 as u64|\\.0 as wasi|\\.0 as usize' path=. mode=content`

## poetry

**kdb** — 34s, 7 turns, $0.14
  1. `Bash: kdb tree -L 2 2>/dev/null | head -80`
  2. `Grep: pattern='class RepositoryPool' path=. mode=files_with_matches`
  3. `Grep: pattern='RepositoryPool' path=. mode=files_with_matches`
  4. `Bash: kdb symbols src/poetry/repositories/repository_pool.py -s RepositoryPool`
  5. `Bash: kdb refs src/poetry/repositories/repository_pool.py -s RepositoryPool`
  6. `Grep: pattern='RepositoryPool\\(' path=. mode=content`

**baseline** — 25s, 5 turns, $0.09
  1. `Grep: pattern='class RepositoryPool' path=. mode=content`
  2. `Grep: pattern='RepositoryPool' path=. mode=files_with_matches`
  3. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/repositories/repository_pool.py`
  4. `Grep: pattern='RepositoryPool\\(' path=. mode=content`

## tokio

**kdb** — 75s, 22 turns, $0.28
  1. `Bash: kdb tree -L 2 2>/dev/null | head -80`
  2. `Grep: pattern='trait AsyncRead' path=. mode=files_with_matches`
  3. `Grep: pattern='fn poll_read' path=. mode=files_with_matches`
  4. `Grep: pattern='\\.poll_read\\(' path=. mode=files_with_matches`
  5. `Grep: pattern='impl.*AsyncRead.*for' path=. mode=content`
  6. `Grep: pattern='poll_read' path=tokio/src/io/bsd mode=files_with_matches`
  7. `Grep: pattern='poll_read' path=tokio/src/io/poll_evented.rs mode=files_with_matches`
  8. `Grep: pattern='poll_read' path=tokio/src/runtime mode=files_with_matches`
  9. `Grep: pattern='poll_read' path=tokio-util/src/codec mode=files_with_matches`
  10. `Grep: pattern='poll_read' path=tokio-util/src/sync mode=files_with_matches`
  11. `Grep: pattern='poll_read' path=tokio/tests mode=files_with_matches`
  12. `Grep: pattern='poll_read' path=tokio-util/tests mode=files_with_matches`
  13. `Grep: pattern='poll_read' path=tokio/src/io/async_fd.rs mode=files_with_matches`
  14. `Grep: pattern='poll_read' path=tokio-util/src/io/copy_to_bytes.rs mode=files_with_matches`
  15. `Grep: pattern='poll_read' path=tokio-util/src/udp mode=files_with_matches`
  16. `Grep: pattern='poll_read' path=tokio-util/src/io/sync_bridge.rs mode=files_with_matches`
  17. `Grep: pattern='poll_read' path=tokio-util/src/io/reader_stream.rs mode=files_with_matches`
  18. `Grep: pattern='poll_read' path=tokio-test mode=files_with_matches`
  19. `Grep: pattern='poll_read' path=benches mode=files_with_matches`
  20. `Grep: pattern='poll_read' path=tokio/tests/sync_mpsc.rs mode=files_with_matches`
  21. `Grep: pattern='poll_read' path=. mode=files_with_matches`

**baseline** — 36s, 4 turns, $0.11
  1. `Grep: pattern='fn poll_read\\(' path=. mode=content`
  2. `Grep: pattern='\\.poll_read\\(' path=. mode=content`
  3. `Grep: pattern='poll_read' path=. mode=files_with_matches`

