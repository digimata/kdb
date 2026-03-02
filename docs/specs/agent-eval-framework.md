---
path: qmd/docs/specs/agent-eval-framework.md
outline: |
  • Spec: Agent Eval Framework (Open-ended, Speed-of-light)        L30
    ◦ Goals                                                        L40
    ◦ Non-goals                                                    L48
    ◦ Definitions                                                  L54
    ◦ Speed-of-light model (abstract information primitives)       L65
      ▪ Lower bound definition                                     L82
    ◦ Benchmark design rules                                       L97
      ▪ 1) Ticket-shaped prompts                                   L99
      ▪ 2) Bounded deliverable contracts                          L109
      ▪ 3) Anchors for objective scoring                          L118
      ▪ 4) Evidence requirement (anti-hallucination)              L133
    ◦ Scoring                                                     L142
      ▪ Correctness                                               L144
      ▪ Efficiency                                                L153
      ▪ Speed-of-light gap                                        L163
    ◦ Task families (open-ended but scoreable)                    L171
      ▪ F1: RCA from symptom (log / stack / failing test)         L176
      ▪ F2: Trace a user action end-to-end                        L197
      ▪ F3: Add a config/flag with propagation                    L214
      ▪ F4: Backwards-compatible API change (plan-only)           L231
      ▪ F5: Security boundary check                               L252
    ◦ Repository and run hygiene                                  L268
    ◦ Suggested on-disk task layout                               L277
    ◦ Next steps                                                  L300
---

# Spec: Agent Eval Framework (Open-ended, Speed-of-light)

This document defines a framework for designing and scoring open-ended agent
benchmarks that are representative of real engineering work, while still being
measurable and comparable across tools/variants.

The key idea: every benchmark should be describable in terms of the minimum
information that must be obtained from the repo ("speed of light"), independent
of any particular navigation tool.

## Goals

- Realistic prompts: ticket-shaped, not synthetic "find X" microtasks.
- Bounded deliverables: the agent must know what "done" means.
- Objective scoring: correctness measured against anchors, not vibes.
- Efficiency metrics: tool/actions, wall time, and cost.
- Speed-of-light baselines: lower bounds on required information gathering.

## Non-goals

- Ranking LLMs in the abstract.
- Measuring writing style or verbosity.
- Proving semantic completeness for dynamic language reflection/macro expansion.

## Definitions

- Task: a prompt + a deliverable contract + a scoring spec.
- Run: one execution of a task against one repo at one pinned revision.
- Variant: a tool/system-prompt configuration (e.g. kdb-first, grep/glob-only).
- Action: one repo-boundary query/fetch that returns new information.
  - In practice this is usually a tool call, but the framework is abstract.
- Anchor: an objective fact we expect in a correct answer (file, symbol,
  boundary artifact, relationship).
- Evidence: a citation that justifies an anchor (snippet, definition, or link).

## Speed-of-light model (abstract information primitives)

We model information gathering in terms of a small set of primitives. A tool may
implement one primitive directly, or bundle multiple primitives into a single
query.

| ID | Primitive | What it returns | Notes |
|---|---|---|---|
| A0 | Index build / whole-repo scan | a queryable model of the repo | amortizable; required for worst-case completeness |
| A1 | Subtree inventory | list of files under a directory | may include sizes, languages, test files |
| A2 | Symbol identity resolution | unique symbol ID from (file, name) | disambiguates collisions, aliasing |
| A3 | Cross-reference query | semantic refs to a symbol/method/field | ideally includes usage-kind (call vs def vs field) |
| A4 | Type relation query | implementors/overrides/subclasses | requires type graph metadata |
| A5 | Flow slice query | call graph / dependency slice from an entrypoint | can be static over-approx |
| A6 | Boundary mapping query | schema/db/json/codegen edges for a type/API | also "wire format" and migrations |
| A7 | Evidence fetch | focused snippets / definitions | used to justify claims |

### Lower bound definition

For a task, define the minimum set of primitives needed to satisfy the
deliverable contract with the required confidence. The speed-of-light action
count is the size of the smallest set of primitives whose outputs cover the
anchors.

We usually track two bounds:

- Warm bound: assumes A0 already exists (index is built).
- Cold bound: includes A0 when global knowledge is required.

Important: "open-ended" does not mean "unbounded". The contract determines the
minimum information required.

## Benchmark design rules

### 1) Ticket-shaped prompts

Prompts should look like work:

- symptom (error/log/test failure) OR goal (feature/change)
- context (subsystem, constraints, backwards-compat requirements)
- asked deliverable (what to return)

Avoid prompts that leak the implementation path ("run grep for X").

### 2) Bounded deliverable contracts

Every task must state what counts as done. Examples:

- "Identify entrypoint, control-flow spine (5-10 steps), key data types, and 2
  candidate edit points. Provide citations for each." (read-only)
- "Propose a minimal patch plan: list files to edit and tests to run." (plan)
- "Implement the patch and run tests." (patch)

### 3) Anchors for objective scoring

Anchors are the scoring spine. Each task should define:

- must_mention anchors (required for correctness)
- should_mention anchors (bonus / partial credit)
- must_not_mention anchors (common false-positive namespaces)

Anchor types:

- Files: `pkg/scheduler/schedule_one.go`
- Symbols: `Framework`, `RepositoryPool.__init__`, `AsyncRead::poll_read`
- Boundaries: `proto/*.proto`, migrations, generated code
- Relationships: "A calls B", "X implements Y", "type T is serialized via S"

### 4) Evidence requirement (anti-hallucination)

For open-ended tasks, require a small number of citations:

- Each must_mention anchor must have >= 1 evidence citation OR be directly
  supported by a primitive output (e.g. A3/A4 results).
- Evidence budget should be bounded (e.g. 3-8 citations total) to discourage
  reading the entire repo.

## Scoring

### Correctness

Compute:

- Anchor recall: mentioned_required / required_total
- Anchor precision: mentioned_required / mentioned_total (penalize noise)
- Forbidden hit rate: forbidden_mentions / forbidden_total
- Evidence coverage: required_with_citation / required_total

### Efficiency

Report:

- Tool/action count
- Turns
- Wall time
- (Optional) bytes read / files touched
- Redundancy: repeated inventory/grep of the same surface area

### Speed-of-light gap

For each task family, compute an estimated warm lower bound L_warm and compare:

- efficiency_ratio = actual_actions / L_warm

This ratio is what you optimize with better tools/prompts.

## Task families (open-ended but scoreable)

Each family below includes a suggested deliverable contract, anchor style, and
typical speed-of-light primitives.

### F1: RCA from symptom (log / stack / failing test)

Prompt template:

- "When running X, we see error Y. Find likely root cause and where to fix."

Deliverable contract:

- Hypothesis for root cause
- The control-flow path leading to the error (5-10 steps)
- The 1-2 primary fix points (files/symbols)
- 3-5 citations

Typical primitives:

- A2 (resolve symbols in stack/log)
- A5 (flow slice)
- A7 (evidence snippets)

Warm bound: ~3 actions

### F2: Trace a user action end-to-end

Prompt template:

- "When the user does X (CLI/API/UI), what happens? Where would you hook Z?"

Deliverable:

- Entry point(s)
- Control-flow spine (5-10 steps)
- Key types and boundaries
- Hook point and rationale

Typical primitives: A1 + A5 + A7 (+ A6 if boundaries matter)

Warm bound: ~3-5 actions

### F3: Add a config/flag with propagation

Prompt template:

- "Add knob K. Where is config defined, parsed, defaulted, and threaded?"

Deliverable:

- Definition point (schema/flag/env)
- Parse point
- Threading path to consumer(s)
- Tests to update

Typical primitives: A6 + A3 + A7 (+ A5 if flow matters)

Warm bound: ~3-6 actions

### F4: Backwards-compatible API change (plan-only)

Prompt template:

- "We need to change API surface S but keep compatibility. What is impacted?"

Deliverable:

- Mechanical blast radius (compile/typecheck)
- Boundary blast radius (wire/db/codegen)
- Compatibility strategy (defaulting, overloads, adapters)

Typical primitives:

- A3 (call sites / field usage)
- A4 (implementors/overrides)
- A6 (schema/codegen/db)
- A7 (evidence)

Warm bound: ~2-6 actions (depends on whether A6 exists)

### F5: Security boundary check

Prompt template:

- "What prevents unauthorized access to X? Where to enforce if missing?"

Deliverable:

- Enforcement points
- Policy source of truth
- Likely bypasses

Typical primitives: A5 + A7 (+ A3 to find all entrypoints)

Warm bound: ~3-6 actions

## Repository and run hygiene

- Pin each repo to a commit SHA for deterministic anchors.
- Forbid network access.
- Separate warm and cold runs:
  - cold: include A0 cost (index build)
  - warm: pre-build index once per repo/sha
- Store run artifacts (jsonl, summary json, score json) in the task directory.

## Suggested on-disk task layout

Example:

```
benchmarks/agent-realworld/
  f1-rca/
    prompt.md
    anchors/
      tokio.json
      kubernetes.json
    score.py
    results.md
  f2-trace/
    ...
```

Minimum required files per task:

- `prompt.md`: per-repo prompt variants
- `anchors/<repo>.json`: anchors + forbidden namespaces
- `score.py`: scorer that outputs a machine-readable score

## Next steps

1. Pick 2 families (recommend F1 + F3) and draft one prompt per existing repo.
2. Define an initial anchor schema (`anchors/<repo>.json`) and a simple scorer
   (file/symbol mentions + required citations).
3. Run a small calibration set to validate that anchors are neither too strict
   nor too vague, then expand.
