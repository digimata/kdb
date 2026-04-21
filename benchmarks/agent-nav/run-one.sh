#!/usr/bin/env bash
set -euo pipefail

RESULTS_DIR="$(cd "$(dirname "$0")" && pwd)/results"
REPOS_DIR="$HOME/Documents/repos"
mkdir -p "$RESULTS_DIR"

REPO="$1"      # e.g. mio
TASK="$2"      # e.g. t1
VARIANT="$3"   # kdb or baseline

# ── prompts ──

declare -A T1_PROMPTS
T1_PROMPTS[mio]="Find all files that use the Token struct from src/token.rs"
T1_PROMPTS[poetry]="Find all files that use the RepositoryPool class from src/poetry/repositories/repository_pool.py"
T1_PROMPTS[tokio]="Find all files that use the AsyncRead trait from tokio/src/io/async_read.rs"
T1_PROMPTS[airstore]="Find all files that use the Task struct from pkg/types/task.go"
T1_PROMPTS[kubernetes]="Find all files that use the Framework interface from pkg/scheduler/framework/interface.go"

declare -A T2_PROMPTS
T2_PROMPTS[mio]="Explain the architecture of src/sys/ — what platform abstraction pattern does it use?"
T2_PROMPTS[poetry]="Explain the architecture of src/poetry/installation/ — what's the pipeline?"
T2_PROMPTS[tokio]="Explain the architecture of tokio/src/runtime/task/ — how do tasks work?"
T2_PROMPTS[airstore]="Explain the architecture of pkg/worker/ — how does task execution work?"
T2_PROMPTS[kubernetes]="Explain the architecture of pkg/scheduler/ — how does scheduling work?"

declare -A T3_PROMPTS
T3_PROMPTS[mio]="The Token struct needs a new priority: u8 field. List all files that would need updating."
T3_PROMPTS[poetry]="The RepositoryPool.__init__ needs a new timeout parameter. List all call sites that would need updating."
T3_PROMPTS[tokio]="The AsyncRead::poll_read method needs a new deadline: Option<Instant> parameter. List all implementations and call sites."
T3_PROMPTS[airstore]="The Task struct needs a new Priority int field. List all files that construct or read Task that would need updating."
T3_PROMPTS[kubernetes]="The Framework interface needs a new Healthy() bool method. List all implementors that would need updating."

case "$TASK" in
    t1) PROMPT="${T1_PROMPTS[$REPO]}" ;;
    t2) PROMPT="${T2_PROMPTS[$REPO]}" ;;
    t3) PROMPT="${T3_PROMPTS[$REPO]}" ;;
    *) echo "Unknown task: $TASK"; exit 1 ;;
esac

FULL_PROMPT="$PROMPT

Return: (1) the complete list of files found, (2) total count, (3) a numbered list of every tool call you made (tool name + arguments) in order.

Do NOT edit any files. Do NOT commit. This is a read-only research task."

OUTFILE="$RESULTS_DIR/${REPO}-${TASK}-${VARIANT}.jsonl"

# ── system prompt for kdb variant ──

read -r -d '' KDB_SYSTEM_PROMPT << 'SYSPROMPT' || true
## Navigation

Prefer `kdb` over Glob/Grep for navigating projects. All commands work on both markdown and code files. Run `kdb init` at the project root if no `.kdb` directory exists.

| Task | Use |
|---|---|
| List symbols in a file (headings, functions, types, etc.) | `kdb symbols <file>` |
| Get specific symbol(s) | `kdb symbols <file> -s <name>...` |
| Find who imports a code symbol | `kdb refs <file> -s <symbol>` |
| Find inbound links to a markdown file/heading | `kdb refs <target>` |
| List outbound deps (links from md, imports from code) | `kdb deps <file>` |
| Explore project/directory structure | `kdb tree [path] [-L <depth>]` |
| Find broken links / orphans | `kdb check` |

Fall back to Grep/Glob when: searching for arbitrary strings/patterns, or kdb doesn't cover the query (e.g. regex search across file contents). Note: `kdb refs` is still maturing — if results look incomplete, verify with Grep.

```text
kdb
  init                Initialize a kdb project in a directory
  check [PATH]        Report broken links and orphan files
  tree [PATH]         Print a filtered directory tree
                      [-L <depth>] [-a] [-d] [-f] [-I <glob>] [-P <glob>] [-J]
  symbols <PATH>      Print symbols for a markdown or code file
                      [-s <name>...] [--json] [--public]
  refs <TARGET>       Find inbound references to a markdown target or code symbol
                      [-s <symbol>] [-c <N>] [--json] [--count]
  deps <TARGET>       Print direct dependencies for a file/symbol target
                      [--json]
  graph [PATH]        Render a dependency graph
  fmt [PATH]          Generate or update code index headers
  lsp                 Run the language server over stdio
```
SYSPROMPT

# ── run ──

cd "$REPOS_DIR/$REPO"

START=$(date +%s)

if [[ "$VARIANT" == "kdb" ]]; then
    claude --print \
        --output-format stream-json \
        --model sonnet \
        --no-session-persistence \
        --system-prompt "$KDB_SYSTEM_PROMPT" \
        --allowedTools "Bash(kdb:*) Read Glob Grep" \
        "$FULL_PROMPT" \
        > "$OUTFILE" 2>&1
else
    claude --print \
        --output-format stream-json \
        --model sonnet \
        --no-session-persistence \
        --allowedTools "Grep Glob Read" \
        "$FULL_PROMPT" \
        > "$OUTFILE" 2>&1
fi

END=$(date +%s)
ELAPSED=$((END - START))

echo "WALL_TIME=${ELAPSED}s" | tee "$RESULTS_DIR/${REPO}-${TASK}-${VARIANT}.time"
echo ""
echo "Output: $OUTFILE"
