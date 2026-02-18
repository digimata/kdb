---
id: 10
title: Show Header Titles in Completion
status: done
priority: high
labels:
  - fix
---

# 0010 :: Show Header Titles in Completion

## Intent

Heading completions should display the actual header title as the label, not the slug.

## Current Behavior

Completions show the slug followed by the title:

```
baseview-definitions Base/View Definitions
done-when Done When
open-questions Open Questions
```

The slug is an implementation detail — users think in terms of the heading text, not the anchor.

## Desired Behavior

Show the heading title as the completion label, and insert the slug on accept:

```
Base/View Definitions
Done When
Open Questions
```

The `insert_text` (or `text_edit`) on the completion item should still insert the slug since that's what the link syntax needs.
