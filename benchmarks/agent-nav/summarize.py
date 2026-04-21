#!/usr/bin/env python3
"""Extract a clean summary JSON from a benchmark .jsonl file."""

import json
import sys
import os


# ----------------------
# projects/kdb/benchmarks/agent-nav/summarize.py
#
# def summarize()    L15
# ----------------------

def summarize(jsonl_path, wall_time_s=None):
    lines = open(jsonl_path).read().strip().split('\n')

    tool_calls = []
    final_text = ""
    result_entry = None

    for line in lines:
        entry = json.loads(line)
        t = entry.get('type')

        if t == 'assistant':
            for c in entry.get('message', {}).get('content', []):
                if c.get('type') == 'tool_use':
                    tool = c['name']
                    inp = c.get('input', {})
                    if tool == 'Bash':
                        tool_calls.append(f"Bash: {inp.get('command', '')[:150]}")
                    elif tool == 'Grep':
                        tool_calls.append(f"Grep: pattern={inp.get('pattern')!r} path={inp.get('path', '.')} mode={inp.get('output_mode', 'files')}")
                    elif tool == 'Glob':
                        tool_calls.append(f"Glob: pattern={inp.get('pattern')!r}")
                    elif tool == 'Read':
                        tool_calls.append(f"Read: {inp.get('file_path', '?')}")
                    else:
                        tool_calls.append(f"{tool}: {str(inp)[:100]}")
                elif c.get('type') == 'text':
                    final_text = c['text']

        elif t == 'result':
            result_entry = entry

    if not result_entry:
        return None

    summary = {
        "duration_ms": result_entry.get("duration_ms"),
        "wall_time_s": wall_time_s,
        "turns": result_entry.get("num_turns"),
        "cost_usd": round(result_entry.get("total_cost_usd", 0), 4),
        "output_tokens": result_entry.get("usage", {}).get("output_tokens"),
        "permission_denials": len(result_entry.get("permission_denials", [])),
        "tool_calls": tool_calls,
        "tool_call_count": len(tool_calls),
        "answer_preview": final_text[:500],
    }

    return summary


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: summarize.py <file.jsonl> [wall_time_s]")
        sys.exit(1)

    path = sys.argv[1]
    wall_time = int(sys.argv[2]) if len(sys.argv) > 2 else None

    summary = summarize(path, wall_time)
    if summary:
        # Write summary.json next to the jsonl
        out_path = path.replace('.jsonl', '.json')
        with open(out_path, 'w') as f:
            json.dump(summary, f, indent=2)
        print(json.dumps(summary, indent=2))
    else:
        print("No result entry found", file=sys.stderr)
        sys.exit(1)
