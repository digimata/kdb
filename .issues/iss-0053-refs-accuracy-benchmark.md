---
id: 53
title: "refs -s accuracy benchmark"
status: proposed
priority: high
labels:
  - research
  - refs
path: qmd/.issues/iss-0053-refs-accuracy-benchmark.md
outline: |
  • ISS-0053 :: refs -s accuracy benchmark      L18
    ◦ Intent                                    L20
    ◦ Approach                                  L24
    ◦ Repos                                     L32
    ◦ Deliverables                              L43
---

# ISS-0053 :: `refs -s` accuracy benchmark

## Intent

Thoroughly benchmark `refs -s` accuracy across all four supported languages (Rust, TypeScript/JS, Python, Go) on real-world codebases. The eval suite (iss-0039) tested specific patterns in synthetic fixtures — this issue is about measuring real-world precision and recall at scale.

## Approach

1. **Inventory** — catalog every code path and heuristic in the `refs -s` pipeline: usage scanning, import resolution, re-export following, alias tracking, namespace access, wildcard imports, etc. Understand exactly what it can and can't resolve.

2. **Benchmark** — for each language, pick 1-2 real-world repos and sample symbols across a range of complexities (simple function calls, method access, re-exported types, aliased imports, etc.). Compare `kdb refs -s` results against ground truth (IDE find-references or manual inspection).

3. **Report** — precision/recall per language, breakdown by reference category, identified blind spots.

## Repos

Pick repos that exercise real-world import patterns:

| Language | Candidates |
|---|---|
| Rust | kdb itself, tokio, axum |
| TS/JS | next.js app, zod, trpc |
| Python | poetry, fastapi, httpx |
| Go | kubernetes (subset), cobra, chi |

## Deliverables

- Accuracy report with precision/recall per language
- List of failure categories with examples
- Recommendations for follow-up fixes (if any)
