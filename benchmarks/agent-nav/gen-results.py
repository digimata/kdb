#!/usr/bin/env python3
"""Generate a combined results.md from summary JSONs in a task directory."""

import json
import os
import sys


# -------------------------
# projects/kdb/benchmarks/agent-nav/gen-results.py
#
# def load_summary()    L16
# def main()            L23
# -------------------------

def load_summary(path):
    try:
        return json.load(open(path))
    except (FileNotFoundError, json.JSONDecodeError):
        return None


def main(task_dir):
    repos = sorted([d for d in os.listdir(task_dir)
                    if os.path.isdir(os.path.join(task_dir, d))])

    lines = []
    lines.append(f"# {os.path.basename(task_dir)}\n")

    # summary table — one row per repo, kdb vs baseline side by side
    lines.append("| Repo | kdb wall | kdb turns | kdb calls | kdb cost | base wall | base turns | base calls | base cost | speedup |")
    lines.append("|---|---|---|---|---|---|---|---|---|---|")

    all_summaries = {}
    for repo in repos:
        k = load_summary(os.path.join(task_dir, repo, "kdb.json"))
        b = load_summary(os.path.join(task_dir, repo, "baseline.json"))
        if not k or not b:
            continue
        all_summaries[repo] = (k, b)

        kw = k.get('wall_time_s', '?')
        bw = b.get('wall_time_s', '?')
        speedup = f"{bw/kw:.1f}x" if isinstance(kw, (int, float)) and isinstance(bw, (int, float)) and kw > 0 else "?"

        lines.append(f"| {repo} | {kw}s | {k['turns']} | {k['tool_call_count']} | ${k['cost_usd']:.2f} | {bw}s | {b['turns']} | {b['tool_call_count']} | ${b['cost_usd']:.2f} | {speedup} |")

    # averages
    kdb_walls = [s[0].get('wall_time_s', 0) for s in all_summaries.values()]
    base_walls = [s[1].get('wall_time_s', 0) for s in all_summaries.values()]
    kdb_turns = [s[0]['turns'] for s in all_summaries.values()]
    base_turns = [s[1]['turns'] for s in all_summaries.values()]
    kdb_calls = [s[0]['tool_call_count'] for s in all_summaries.values()]
    base_calls = [s[1]['tool_call_count'] for s in all_summaries.values()]
    kdb_costs = [s[0]['cost_usd'] for s in all_summaries.values()]
    base_costs = [s[1]['cost_usd'] for s in all_summaries.values()]

    n = len(all_summaries)
    if n > 0:
        avg_speedup = (sum(base_walls) / sum(kdb_walls)) if sum(kdb_walls) > 0 else 0
        lines.append(f"| **avg** | {sum(kdb_walls)/n:.0f}s | {sum(kdb_turns)/n:.1f} | {sum(kdb_calls)/n:.1f} | ${sum(kdb_costs)/n:.2f} | {sum(base_walls)/n:.0f}s | {sum(base_turns)/n:.1f} | {sum(base_calls)/n:.1f} | ${sum(base_costs)/n:.2f} | {avg_speedup:.1f}x |")

    lines.append("")

    # per-repo tool call detail
    for repo, (k, b) in all_summaries.items():
        lines.append(f"## {repo}\n")
        for variant, s in [("kdb", k), ("baseline", b)]:
            denials = s['permission_denials']
            denial_str = f" ({denials} denied)" if denials else ""
            lines.append(f"**{variant}** — {s.get('wall_time_s', '?')}s, {s['turns']} turns, ${s['cost_usd']:.2f}{denial_str}")
            for i, tc in enumerate(s['tool_calls'], 1):
                lines.append(f"  {i}. `{tc}`")
            lines.append("")

    out_path = os.path.join(task_dir, "results.md")
    with open(out_path, 'w') as f:
        f.write('\n'.join(lines) + '\n')

    print(f"Written to {out_path}")
    print('\n'.join(lines))


if __name__ == '__main__':
    main(sys.argv[1])
