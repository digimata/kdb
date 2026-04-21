#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="$SCRIPT_DIR/results"
REPOS_DIR="$HOME/Documents/repos"
mkdir -p "$RESULTS_DIR"

KDB_SYSTEM_PROMPT='## Navigation

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

Fall back to Grep/Glob when: searching for arbitrary strings/patterns, or kdb doesn'"'"'t cover the query (e.g. regex search across file contents). Note: `kdb refs` is still maturing — if results look incomplete, verify with Grep.

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
```'

# ── task definitions ──

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

REPOS=(mio airstore poetry tokio kubernetes)

# ── runner ──

run_agent() {
    local repo="$1" task="$2" variant="$3" prompt="$4"
    local outfile="$RESULTS_DIR/${repo}-${task}-${variant}.jsonl"
    local timefile="$RESULTS_DIR/${repo}-${task}-${variant}.time"
    local repo_dir="$REPOS_DIR/$repo"

    echo "[$variant] $repo/$task — starting"

    local full_prompt="$prompt

Return: (1) the complete list of files found, (2) total count, (3) a numbered list of every tool call you made (tool name + arguments) in order.

Do NOT edit any files. Do NOT commit. This is a read-only research task."

    local -a cmd_args=(
        claude --print
        --output-format stream-json
        --model sonnet
        --no-session-persistence
    )

    if [[ "$variant" == "kdb" ]]; then
        cmd_args+=(
            --system-prompt "$KDB_SYSTEM_PROMPT"
            --allowedTools "Bash(kdb:*) Read Glob Grep"
        )
    else
        cmd_args+=(
            --allowedTools "Grep Glob Read"
        )
    fi

    # run from within the repo directory, capture wall time
    local start_s
    start_s=$(date +%s)

    (cd "$repo_dir" && "${cmd_args[@]}" "$full_prompt") > "$outfile" 2>&1 || true

    local end_s
    end_s=$(date +%s)
    local elapsed=$(( end_s - start_s ))
    echo "$elapsed" > "$timefile"

    echo "[$variant] $repo/$task — done (${elapsed}s)"
}

# ── main ──

TASK_FILTER="${1:-all}"   # t1, t2, t3, or all
REPO_FILTER="${2:-all}"   # repo name or all

for repo in "${REPOS[@]}"; do
    [[ "$REPO_FILTER" != "all" && "$REPO_FILTER" != "$repo" ]] && continue

    for task_id in t1 t2 t3; do
        [[ "$TASK_FILTER" != "all" && "$TASK_FILTER" != "$task_id" ]] && continue

        # get prompt for this task/repo
        case "$task_id" in
            t1) prompt="${T1_PROMPTS[$repo]}" ;;
            t2) prompt="${T2_PROMPTS[$repo]}" ;;
            t3) prompt="${T3_PROMPTS[$repo]}" ;;
        esac

        # run kdb and baseline in parallel
        run_agent "$repo" "$task_id" "kdb" "$prompt" &
        run_agent "$repo" "$task_id" "baseline" "$prompt" &
        wait
    done
done

echo ""
echo "All runs complete. Results in $RESULTS_DIR/"
