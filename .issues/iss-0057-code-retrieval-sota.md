---
id: 57
title: "code retrieval SOTA research"
status: proposed
priority: medium
labels:
  - research
path: qmd/.issues/iss-0057-code-retrieval-sota.md
outline: |
  • ISS-0057 :: Code retrieval SOTA research      L16
    ◦ Intent                                      L18
    ◦ Scope                                       L22
    ◦ Deliverables                                L34
---

# ISS-0057 :: Code retrieval SOTA research

## Intent

Survey the current state of the art (as of Feb 2026) for code retrieval — the techniques, models, and systems used to find relevant code given a natural-language query or code context. This directly informs how kdb should index and surface code for agent-assisted workflows.

## Scope

1. **Embedding-based retrieval** — code embedding models (e.g. OpenAI code-search, Voyage Code, Jina Code, Nomic), vector DB strategies, chunking approaches for code.

2. **Symbolic / structural retrieval** — AST-based indexing, scope-aware search, type-directed lookup. How do tools like Sourcegraph, GitHub code search, and language servers approach this?

3. **Hybrid approaches** — combining lexical (BM25/trigram), semantic (embedding), and structural (tree-sitter/LSP) signals. Reranking strategies.

4. **Agentic retrieval** — how do coding agents (Cursor, Copilot, Cline, Aider, Claude Code) retrieve context today? What's known about their RAG pipelines?

5. **Benchmarks** — what benchmarks exist for code retrieval (CodeSearchNet, CoSQA, SWE-bench retrieval components, etc.)? What do SOTA numbers look like?

## Deliverables

- Research summary document covering each area above
- Comparison matrix of approaches (tradeoffs, maturity, applicability to kdb)
- Recommendations for kdb's retrieval strategy
