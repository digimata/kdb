---
path: projects/kdb/benchmarks/agent-nav/t2-explain-arch/results.md
outline: |
  • t2-explain-arch       L12
    ◦ airstore            L23
    ◦ kubernetes          L48
    ◦ mio                 L87
    ◦ poetry             L109
    ◦ tokio              L139
---

# t2-explain-arch

| Repo | kdb wall | kdb turns | kdb calls | kdb cost | base wall | base turns | base calls | base cost | speedup |
|---|---|---|---|---|---|---|---|---|---|
| airstore | 62s | 11 | 10 | $0.43 | 52s | 10 | 9 | $0.38 | 0.8x |
| kubernetes | 120s | 14 | 13 | $0.54 | 163s | 2 | 20 | $0.39 | 1.4x |
| mio | 48s | 8 | 7 | $0.19 | 46s | 10 | 9 | $0.18 | 1.0x |
| poetry | 51s | 13 | 12 | $0.20 | 51s | 13 | 12 | $0.28 | 1.0x |
| tokio | 91s | 11 | 10 | $0.31 | 121s | 2 | 26 | $0.29 | 1.3x |
| **avg** | 74s | 11.4 | 10.4 | $0.33 | 87s | 7.4 | 15.2 | $0.30 | 1.2x |

## airstore

**kdb** — 62s, 11 turns, $0.43
  1. `Bash: kdb tree pkg/worker -L 3`
  2. `Bash: ls -la pkg/worker/`
  3. `Bash: kdb symbols pkg/worker/worker.go pkg/worker/sandbox.go pkg/worker/interactive_task.go pkg/worker/image.go pkg/worker/mount.go pkg/worker/network.go pk`
  4. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/worker.go`
  5. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/sandbox.go`
  6. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/interactive_task.go`
  7. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/output.go`
  8. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/mount.go`
  9. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/network.go`
  10. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/image.go`

**baseline** — 52s, 10 turns, $0.38
  1. `Glob: pattern='pkg/worker/**/*'`
  2. `Bash: ls -la /Users/andjones/Documents/repos/airstore/pkg/worker/`
  3. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/worker.go`
  4. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/sandbox.go`
  5. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/image.go`
  6. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/interactive_task.go`
  7. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/mount.go`
  8. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/network.go`
  9. `Read: /Users/andjones/Documents/repos/airstore/pkg/worker/output.go`

## kubernetes

**kdb** — 120s, 14 turns, $0.54
  1. `Bash: kdb tree pkg/scheduler/ -L 3`
  2. `Bash: kdb symbols pkg/scheduler/ 2>/dev/null || echo "Need individual files"`
  3. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/scheduler.go`
  4. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/schedule_one.go`
  5. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/interface.go`
  6. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/backend/queue/scheduling_queue.go`
  7. `Read: /Users/andjones/.claude/projects/-Users-andjones-Documents-repos-kubernetes/3acc7ef5-9a20-4aa6-b91c-03911db13222/tool-results/toolu_019nCCbk3GeGmmRFK4oNguPX.txt`
  8. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/eventhandlers.go`
  9. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/backend/cache/interface.go`
  10. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/runtime/framework.go`
  11. `Read: /Users/andjones/.claude/projects/-Users-andjones-Documents-repos-kubernetes/3acc7ef5-9a20-4aa6-b91c-03911db13222/tool-results/toolu_019nCCbk3GeGmmRFK4oNguPX.txt`
  12. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/plugins/registry.go`
  13. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/profile/profile.go`

**baseline** — 163s, 2 turns, $0.39
  1. `Task: {'description': 'Explore pkg/scheduler architecture', 'subagent_type': 'Explore', 'prompt': 'Thoroug`
  2. `Glob: pattern='pkg/scheduler/**/*'`
  3. `Bash: find /Users/andjones/Documents/repos/kubernetes/pkg/scheduler -type f | wc -l`
  4. `Bash: find /Users/andjones/Documents/repos/kubernetes/pkg/scheduler -type f | sort`
  5. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/scheduler.go`
  6. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/schedule_one.go`
  7. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/interface.go`
  8. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/backend/queue/scheduling_queue.go`
  9. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/runtime/framework.go`
  10. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/backend/cache/cache.go`
  11. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/plugins/registry.go`
  12. `Grep: pattern='type.*Plugin.*interface|extension point|scheduling cycle' path=/Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework mode=content`
  13. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/eventhandlers.go`
  14. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/types.go`
  15. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/apis/config/types.go`
  16. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/backend/api_dispatcher/api_dispatcher.go`
  17. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/cycle_state.go`
  18. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/framework/preemption/preemption.go`
  19. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/schedule_one.go`
  20. `Read: /Users/andjones/Documents/repos/kubernetes/pkg/scheduler/scheduler.go`

## mio

**kdb** — 48s, 8 turns, $0.19
  1. `Bash: kdb tree src/sys/ -L 4`
  2. `Glob: pattern='src/sys/**/*'`
  3. `Read: /Users/andjones/Documents/repos/mio/src/sys/mod.rs`
  4. `Read: /Users/andjones/Documents/repos/mio/src/sys/unix/mod.rs`
  5. `Read: /Users/andjones/Documents/repos/mio/src/sys/windows/mod.rs`
  6. `Read: /Users/andjones/Documents/repos/mio/src/sys/shell/mod.rs`
  7. `Read: /Users/andjones/Documents/repos/mio/src/sys/wasi/mod.rs`

**baseline** — 46s, 10 turns, $0.18
  1. `Glob: pattern='src/sys/**/*'`
  2. `Read: /Users/andjones/Documents/repos/mio/src/sys`
  3. `Glob: pattern='src/sys/**/*'`
  4. `Bash: ls -R /Users/andjones/Documents/repos/mio/src/sys/`
  5. `Read: /Users/andjones/Documents/repos/mio/src/sys/mod.rs`
  6. `Read: /Users/andjones/Documents/repos/mio/src/sys/unix/mod.rs`
  7. `Read: /Users/andjones/Documents/repos/mio/src/sys/shell/mod.rs`
  8. `Read: /Users/andjones/Documents/repos/mio/src/sys/windows/mod.rs`
  9. `Read: /Users/andjones/Documents/repos/mio/src/sys/wasi/mod.rs`

## poetry

**kdb** — 51s, 13 turns, $0.20
  1. `Bash: kdb tree src/poetry/installation -L 3`
  2. `Bash: kdb symbols src/poetry/installation/__init__.py 2>/dev/null; echo "---"; ls -1 src/poetry/installation/`
  3. `Bash: kdb symbols src/poetry/installation/installer.py`
  4. `Bash: kdb symbols src/poetry/installation/executor.py`
  5. `Bash: kdb symbols src/poetry/installation/chef.py`
  6. `Bash: kdb symbols src/poetry/installation/chooser.py`
  7. `Bash: kdb symbols src/poetry/installation/wheel_installer.py`
  8. `Bash: kdb symbols src/poetry/installation/operations/operation.py src/poetry/installation/operations/install.py src/poetry/installation/operations/uninstall`
  9. `Bash: kdb symbols src/poetry/installation/installer.py -s run _do_install _execute`
  10. `Bash: kdb symbols src/poetry/installation/executor.py -s execute _execute_operation _do_execute_operation _install _prepare_archive`
  11. `Bash: kdb deps src/poetry/installation/installer.py`
  12. `Bash: kdb deps src/poetry/installation/executor.py`

**baseline** — 51s, 13 turns, $0.28
  1. `Glob: pattern='src/poetry/installation/**/*'`
  2. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/installer.py`
  3. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/executor.py`
  4. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/chef.py`
  5. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/chooser.py`
  6. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/wheel_installer.py`
  7. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/operations/operation.py`
  8. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/operations/install.py`
  9. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/operations/update.py`
  10. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/operations/uninstall.py`
  11. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/__init__.py`
  12. `Read: /Users/andjones/Documents/repos/poetry/src/poetry/installation/operations/__init__.py`

## tokio

**kdb** — 91s, 11 turns, $0.31
  1. `Bash: kdb tree tokio/src/runtime/task/ -L 2`
  2. `Bash: ls -1 tokio/src/runtime/task/`
  3. `Bash: kdb symbols tokio/src/runtime/task/mod.rs tokio/src/runtime/task/core.rs tokio/src/runtime/task/raw.rs tokio/src/runtime/task/state.rs tokio/src/runti`
  4. `Bash: kdb symbols tokio/src/runtime/task/join.rs tokio/src/runtime/task/waker.rs tokio/src/runtime/task/abort.rs tokio/src/runtime/task/error.rs tokio/src/r`
  5. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/mod.rs`
  6. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/raw.rs`
  7. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/state.rs`
  8. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/core.rs`
  9. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/harness.rs`
  10. `Bash: kdb symbols tokio/src/runtime/task/trace/mod.rs tokio/src/runtime/task/trace/symbol.rs tokio/src/runtime/task/trace/tree.rs`

**baseline** — 121s, 2 turns, $0.29
  1. `Task: {'description': 'Explore tokio task architecture', 'subagent_type': 'Explore', 'prompt': "Thoroughly`
  2. `Bash: find /Users/andjones/Documents/repos/tokio/src/runtime/task -type f -name "*.rs" | sort`
  3. `Bash: ls -la /Users/andjones/Documents/repos/tokio/src/runtime/`
  4. `Bash: ls -la /Users/andjones/Documents/repos/tokio/`
  5. `Bash: ls -la /Users/andjones/Documents/repos/tokio/tokio/src/`
  6. `Bash: ls -la /Users/andjones/Documents/repos/tokio/tokio/src/runtime/`
  7. `Bash: find /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task -type f -name "*.rs" | sort`
  8. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/mod.rs`
  9. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/state.rs`
  10. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/raw.rs`
  11. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/core.rs`
  12. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/harness.rs`
  13. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/join.rs`
  14. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/list.rs`
  15. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/waker.rs`
  16. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/abort.rs`
  17. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/error.rs`
  18. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/id.rs`
  19. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/harness.rs`
  20. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/join.rs`
  21. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/list.rs`
  22. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/harness.rs`
  23. `Read: /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/trace/mod.rs`
  24. `Bash: wc -l /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/harness.rs`
  25. `Bash: wc -l /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task/join.rs`
  26. `Bash: find /Users/andjones/Documents/repos/tokio/tokio/src/runtime/task -type f -name "*.rs" | xargs wc -l | tail -1`

