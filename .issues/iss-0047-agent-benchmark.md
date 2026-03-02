---
id: 47
title: "Benchmark AI agent performance with and without kdb"
status: in_progress
priority: medium
labels:
  - research
path: qmd/.issues/iss-0047-agent-benchmark.md
outline: |
  • ISS-0047 :: Benchmark AI agent performance with and without kdb      L19
    ◦ Intent                                                             L21
    ◦ Agents to test                                                     L25
    ◦ Benchmark design                                                   L30
      ▪ Metrics                                                          L34
      ▪ Task candidates                                                  L41
    ◦ Transcript analysis                                                L53
---

# ISS-0047 :: Benchmark AI agent performance with and without kdb

## Intent

Measure whether kdb actually makes AI coding agents faster and cheaper. Run the same tasks with and without kdb available, compare wall time and token usage. Then analyze transcripts to find where agents underuse kdb or use it suboptimally, and adjust prompting accordingly.

## Agents to test

- Claude Code (Claude)
- Codex (OpenAI)

## Benchmark design

Give each agent the same set of tasks across a few repos. Run each task twice: once with kdb installed + CLAUDE.md navigation instructions, once without (baseline grep/glob/find).

### Metrics

- **Wall time** — end-to-end task completion
- **Token usage** — input + output tokens consumed
- **Tool calls** — total number of tool invocations (fewer = more efficient navigation)
- **Correctness** — did the agent produce the right answer / change

### Task candidates

Pick tasks that exercise cross-file navigation (where kdb should shine):

1. "Find all callers of function X" (refs)
2. "What does file Y depend on?" (deps)
3. "Add a new parameter to function X and update all call sites" (refs + edit)
4. "Explain the architecture of module Z" (symbols + deps + tree)
5. "Fix the broken import in file W" (symbols + deps + check)

Run across 2-3 repos of varying size (small ~100 files, medium ~500, large ~5k+).

## Transcript analysis

After benchmarking, review agent session transcripts for:

- **Underuse** — agent falls back to grep/glob when kdb would be faster
- **Misuse** — agent calls kdb with wrong arguments or misinterprets output
- **Missed opportunities** — agent reads entire files when `kdb symbols -s` would suffice
- **Redundant calls** — agent re-navigates to the same information multiple times

Use findings to:

1. Improve CLAUDE.md navigation instructions (clearer when-to-use guidance)
2. Adjust kdb output format if agents consistently misparse it
3. Add kdb commands or flags that would help agents specifically
