#!/usr/bin/env python3
"""Parse benchmark .jsonl files into readable timelines."""

import json
import sys
import os

# ----------------------
# projects/kdb/benchmarks/agent-nav/parse.py
#
# def parse_run()    L14
# ----------------------

def parse_run(path):
    lines = open(path).read().strip().split('\n')
    out = []

    # derive label from path: .../t1-find-callers/mio/kdb.jsonl or mio-t1-kdb.jsonl
    label = os.path.basename(path).replace('.jsonl', '')
    parent = os.path.basename(os.path.dirname(path))
    grandparent = os.path.basename(os.path.dirname(os.path.dirname(path)))
    if parent and grandparent and parent != 'results':
        label = f"{parent}/{label} ({grandparent})"

    out.append(f"{'='*60}")
    out.append(f"  {label}")
    out.append(f"{'='*60}")

    step = 0
    for line in lines:
        entry = json.loads(line)
        t = entry.get('type')

        if t == 'assistant':
            msg = entry.get('message', {})
            for c in msg.get('content', []):
                if c.get('type') == 'thinking':
                    step += 1
                    thinking = c['thinking'][:200].replace('\n', ' ')
                    out.append(f"\n[{step}] THINK: {thinking}")
                elif c.get('type') == 'tool_use':
                    step += 1
                    tool = c['name']
                    inp = c.get('input', {})
                    if tool == 'Bash':
                        cmd = inp.get('command', '')[:120]
                        out.append(f"[{step}] TOOL: Bash — {cmd}")
                    elif tool == 'Grep':
                        out.append(f"[{step}] TOOL: Grep — pattern={inp.get('pattern')!r} path={inp.get('path','.')} mode={inp.get('output_mode','files')}")
                    elif tool == 'Glob':
                        out.append(f"[{step}] TOOL: Glob — pattern={inp.get('pattern')!r} path={inp.get('path','.')}")
                    elif tool == 'Read':
                        out.append(f"[{step}] TOOL: Read — {inp.get('file_path','?')}")
                    else:
                        out.append(f"[{step}] TOOL: {tool} — {str(inp)[:120]}")
                elif c.get('type') == 'text':
                    step += 1
                    txt = c['text'][:400].replace('\n', ' | ')
                    out.append(f"[{step}] TEXT: {txt}")

        elif t == 'user':
            msg = entry.get('message', {})
            contents = msg.get('content', []) if isinstance(msg.get('content'), list) else []
            for c in contents:
                if c.get('type') == 'tool_result':
                    is_err = c.get('is_error', False)
                    content = str(c.get('content', ''))[:200].replace('\n', ' | ')
                    tag = "ERROR" if is_err else "RESULT"
                    out.append(f"     └─ {tag}: {content}")

        elif t == 'result':
            out.append(f"\n--- SUMMARY ---")
            out.append(f"Duration: {entry.get('duration_ms')}ms")
            out.append(f"Turns: {entry.get('num_turns')}")
            out.append(f"Cost: ${entry.get('total_cost_usd', 0):.4f}")
            denials = entry.get('permission_denials', [])
            out.append(f"Permission denials: {len(denials)}")
            u = entry.get('usage', {})
            out.append(f"Output tokens: {u.get('output_tokens', '?')}")

    return '\n'.join(out)


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: parse.py <file.jsonl> [file2.jsonl ...]")
        sys.exit(1)

    results = []
    for path in sys.argv[1:]:
        results.append(parse_run(path))

    output = '\n\n'.join(results)
    print(output)

    # Also write to .txt alongside each jsonl
    for path in sys.argv[1:]:
        txt_path = path.replace('.jsonl', '.txt')
        with open(txt_path, 'w') as f:
            f.write(parse_run(path) + '\n')
