# Prosaic

Prosaic is a pseudocode language for writing operational procedures and SOPs. It uses indentation-based nesting and a small set of highlighted token types.

## Token Types

| Token | Highlight | Examples |
|---|---|---|
| Comment | `@comment` | `/* ... */` |
| Annotation | `@comment.doc` | `(facts, not feelings)` inside comments |
| Control keyword | `@keyword` | `for each`, `if`, `in`, `break` |
| Action verb | `@function` | `copy`, `write`, `create`, `mark`, `list`, `update`, `sort` |
| Block label | `@label` | `scan:`, `output:`, `budget:` |
| File path | `@string.special` | `.tasks/TODO.md`, `.cycle/{id}/plan.md` |
| Template var | `@string.escape` | `{cycle_id}`, `{YYYY.MM.DD}` |
| Operator | `@operator` | `→`, `=`, `×` |

## Usage

Use `prosaic` as the language identifier in fenced code blocks:

````
```prosaic
/* 1. Reconcile tasks */
for each task in TODO.md:
    if completed:
        mark done
    if incomplete:
        carry over to heap
```
````

## Grammar

The tree-sitter grammar lives at `grammars/tree-sitter-prosaic/`.
