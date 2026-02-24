---
id: 15
title: Gate Orphan Listing Behind Flag
status: proposed
priority: medium
labels:
  - quality
---

# ISS-0015 :: Gate Orphan Listing Behind Flag

## Intent

`kdb check` should stay high-signal by default; orphan file listings can be extremely noisy on larger vaults.

## Behavior

- Default `kdb check` output should NOT print the full orphan list.
- Add `kdb check --orphans` (or similar) to explicitly print orphan files.
- Default behavior can still include an orphan count + a hint (e.g. "N orphan files (run `kdb check --orphans` to list)").
- Ensure `kdb orphans` (if/when implemented) remains the dedicated command for listing orphans.

## Notes

- If we support machine-readable output (JSON), include orphan count always, and the list only when the user requests it.
