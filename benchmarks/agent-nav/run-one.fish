#!/usr/bin/env fish

set SCRIPT_DIR (cd (dirname (status filename)); and pwd)
set REPOS_DIR "$HOME/Documents/repos"

set REPO $argv[1]      # e.g. mio
set TASK $argv[2]      # e.g. t1
set VARIANT $argv[3]   # kdb or baseline

# ── prompts ──

switch "$TASK-$REPO"
    # T1 — find callers
    case "t1-mio";        set PROMPT "Find all files that use the Token struct from src/token.rs"
    case "t1-poetry";     set PROMPT "Find all files that use the RepositoryPool class from src/poetry/repositories/repository_pool.py"
    case "t1-tokio";      set PROMPT "Find all files that use the AsyncRead trait from tokio/src/io/async_read.rs"
    case "t1-airstore";   set PROMPT "Find all files that use the Task struct from pkg/types/task.go"
    case "t1-kubernetes";  set PROMPT "Find all files that use the Framework interface from pkg/scheduler/framework/interface.go"
    # T2 — explain architecture
    case "t2-mio";        set PROMPT "Explain the architecture of src/sys/ — what platform abstraction pattern does it use?"
    case "t2-poetry";     set PROMPT "Explain the architecture of src/poetry/installation/ — what's the pipeline?"
    case "t2-tokio";      set PROMPT "Explain the architecture of tokio/src/runtime/task/ — how do tasks work?"
    case "t2-airstore";   set PROMPT "Explain the architecture of pkg/worker/ — how does task execution work?"
    case "t2-kubernetes";  set PROMPT "Explain the architecture of pkg/scheduler/ — how does scheduling work?"
    # T3 — add parameter
    case "t3-mio";        set PROMPT "I want to add a priority: u8 field to the Token struct. What's the blast radius?"
    case "t3-poetry";     set PROMPT "I want to add a timeout parameter to RepositoryPool.__init__. What's the blast radius?"
    case "t3-tokio";      set PROMPT "I want to add a deadline: Option<Instant> parameter to AsyncRead::poll_read. What's the blast radius?"
    case "t3-airstore";   set PROMPT "I want to add a Priority int field to the Task struct. What's the blast radius?"
    case "t3-kubernetes";  set PROMPT "I want to add a Healthy() bool method to the Framework interface. What's the blast radius?"
    case '*'
        echo "Unknown task/repo: $TASK/$REPO"
        exit 1
end

set FULL_PROMPT "$PROMPT

Return: (1) the complete list of files found, (2) total count, (3) a numbered list of every tool call you made (tool name + arguments) in order.

Do NOT edit any files. Do NOT commit. This is a read-only research task."

# map task id to folder name
switch "$TASK"
    case "t1"; set TASK_DIR "$SCRIPT_DIR/t1-find-callers"
    case "t2"; set TASK_DIR "$SCRIPT_DIR/t2-explain-arch"
    case "t3"; set TASK_DIR "$SCRIPT_DIR/t3-add-param"
end

set RESULTS_DIR "$TASK_DIR/$REPO"
mkdir -p "$RESULTS_DIR"
set OUTFILE "$RESULTS_DIR/$VARIANT.jsonl"

# ── system prompt for kdb variant ──

set KDB_SYSTEM_PROMPT '## Navigation

kdb is already initialized. Run kdb commands directly — no error handling wrappers needed. Trust kdb output — do NOT cross-check or verify kdb results with Grep/Glob.

| Task | Use |
|---|---|
| List symbols in file(s) | `kdb symbols <file>...` (e.g. `kdb symbols foo.rs bar.rs baz.rs`) |
| Get specific symbol(s) | `kdb symbols <file>... -s <name1> <name2> ...` |
| Find who imports a code symbol | `kdb refs <file> -s <symbol>` |
| Find inbound links to a markdown file/heading | `kdb refs <target>` |
| List outbound deps (links from md, imports from code) | `kdb deps <file>` |
| Explore project/directory structure | `kdb tree [path] [-L <depth>]` |
| Find broken links / orphans | `kdb check` |

Only fall back to Grep/Glob for arbitrary string/pattern searches that kdb doesn\'t cover (e.g. regex across file contents).

```text
kdb
  init                Initialize a kdb project in a directory
  check [PATH]        Report broken links and orphan files
  tree [PATH]         Print a filtered directory tree
                      [-L <depth>] [-a] [-d] [-f] [-I <glob>] [-P <glob>] [-J]
  symbols <PATH>      Print symbols for a markdown or code file
                      [-s <name>...] [--json] [--public]
  refs <TARGET>       Find inbound references to a markdown target or code symbol
                      [-s <symbol>] [-c <N> context lines] [--files] [--json] [--count]
  deps <TARGET>       Print direct dependencies for a file/symbol target
                      [--json]
  graph [PATH]        Render a dependency graph
  fmt [PATH]          Generate or update code index headers
  lsp                 Run the language server over stdio
```'

# ── run ──

cd "$REPOS_DIR/$REPO"

set START (date +%s)

if test "$VARIANT" = "kdb"
    echo "$FULL_PROMPT" | claude --print \
        --output-format stream-json --verbose \
        --model opus \
        --no-session-persistence \
        --system-prompt "$KDB_SYSTEM_PROMPT" \
        --allowedTools "Bash Read Glob Grep" \
        > "$OUTFILE" 2>&1
else
    echo "$FULL_PROMPT" | claude --print \
        --output-format stream-json --verbose \
        --model opus \
        --no-session-persistence \
        --allowedTools "Bash Read Glob Grep" \
        > "$OUTFILE" 2>&1
end

set END (date +%s)
set ELAPSED (math $END - $START)

# generate summary json
python3 "$SCRIPT_DIR/summarize.py" "$OUTFILE" "$ELAPSED" 2>/dev/null > /dev/null

echo "[$VARIANT] $REPO/$TASK — {$ELAPSED}s"
