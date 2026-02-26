// --------------------------------------
// apps/landing/src/app/page.tsx
//
// const INSTALL_PROMPT               L29
// const SYSTEM_PROMPT_BLOCK          L49
// const CLI_HELP                     L68
// const EXAMPLE_SYMBOLS              L85
// const EXAMPLE_REFS                 L93
// const EXAMPLE_TREE                 L98
// const FEATURES                    L114
// function CopyButton()             L166
// label                             L166
// text                              L166
// function handleCopy()             L169
// function CodeBlock()              L195
// children                          L199
// copyable                          L200
// function handleCopy()             L204
// function TerminalBlock()          L227
// children                          L227
// export default function Home()    L244
// --------------------------------------

"use client";

import { useState } from "react";
import { GithubIcon, ClipboardIcon, CheckIcon } from "@/components/ui/icons";

const INSTALL_PROMPT = `Install kdb (\`curl -fsSL https://kdb.kernl.sh/install | bash\`) and add the following to your CLAUDE.md:

\`\`\`markdown
## Navigation

Prefer \`kdb\` over Glob/Grep for navigating projects. All commands work on both markdown and code files. Run \`kdb init\` at the project root if no \`.kdb\` directory exists.

| Task | Use |
|---|---|
| List symbols in a file (headings, functions, types, etc.) | \`kdb symbols <file>\` |
| Get specific symbol(s) | \`kdb symbols <file> -s <name>...\` |
| Find who imports a code symbol | \`kdb refs <file> -s <symbol>\` |
| Find inbound links to a markdown file/heading | \`kdb refs <target>\` |
| List outbound deps (links from md, imports from code) | \`kdb deps <file>\` |
| Explore project/directory structure | \`kdb tree [path] [-L <depth>]\` |
| Find broken links / orphans | \`kdb check\` |

Fall back to Grep/Glob when: searching for arbitrary strings/patterns, or kdb doesn't cover the query (e.g. regex search across file contents). Note: \`kdb refs\` is still maturing — if results look incomplete, verify with Grep.
\`\`\``;

const SYSTEM_PROMPT_BLOCK = `## Navigation

Prefer \`kdb\` over Glob/Grep for navigating projects.
All commands work on both markdown and code files.
Run \`kdb init\` at the project root if no \`.kdb\` directory exists.

| Task                          | Use                                  |
|-------------------------------|--------------------------------------|
| List symbols in a file        | \`kdb symbols <file>\`                 |
| Get specific symbol(s)        | \`kdb symbols <file> -s <name>...\`    |
| Find who imports a symbol     | \`kdb refs <file> -s <symbol>\`        |
| Find inbound links to a file  | \`kdb refs <target>\`                  |
| List outbound deps            | \`kdb deps <file>\`                    |
| Explore directory structure   | \`kdb tree [path] [-L <depth>]\`       |
| Find broken links / orphans   | \`kdb check\`                          |

Fall back to Grep/Glob when: searching for arbitrary strings/patterns,
or kdb doesn't cover the query (e.g. regex search across file contents).`;

const CLI_HELP = `kdb
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
  lsp                 Run the language server over stdio`;

/* --------------- terminal example outputs --------------- */

const EXAMPLE_SYMBOLS = `$ kdb symbols src/resolve/mod.rs
src/resolve/mod.rs
  fn resolve_file          pub  L12
  fn resolve_imports       pub  L45
  fn resolve_symbol        pub  L78
  struct ResolveContext     pub  L5
  enum ResolveError        pub  L98`;

const EXAMPLE_REFS = `$ kdb refs src/resolve/mod.rs -s resolve_file
src/index/mod.rs:23         use crate::resolve::resolve_file;
src/cmd.rs:67               let result = resolve_file(&ctx, path)?;
tests/resolve_test.rs:12    use kdb::resolve::resolve_file;`;

const EXAMPLE_TREE = `$ kdb tree -L 2
.
├── docs
│   ├── architecture.md
│   └── getting-started.md
├── src
│   ├── cmd.rs
│   ├── index
│   ├── resolve
│   └── symbols
├── tests
├── Cargo.toml
└── README.md`;

/* --------------- features --------------- */

const FEATURES = [
  {
    name: "symbols",
    title: "Symbols",
    desc: "Extract functions, types, structs, headings from any file. Code and markdown.",
    example: `$ kdb symbols README.md
README.md
  # Getting Started        L1
  ## Installation           L5
  ## Quick Start            L20
  ## Configuration          L45`,
  },
  {
    name: "refs",
    title: "References",
    desc: "Find every file that imports a symbol or links to a document. Instant reverse lookup.",
    example: `$ kdb refs docs/architecture.md
docs/getting-started.md:12    [architecture](architecture.md)
docs/index.md:3               [arch overview](architecture.md)
README.md:48                  [docs](docs/architecture.md)`,
  },
  {
    name: "deps",
    title: "Dependencies",
    desc: "List outbound imports and links. See what a file depends on at a glance.",
    example: `$ kdb deps src/cmd.rs
src/resolve/mod.rs          use crate::resolve::resolve_file
src/index/mod.rs            use crate::index::build_index
src/symbols/query.rs        use crate::symbols::extract`,
  },
  {
    name: "tree",
    title: "Tree",
    desc: "Filtered directory tree. Respects ignore patterns. Shows what matters.",
    example: EXAMPLE_TREE,
  },
  {
    name: "check",
    title: "Check",
    desc: "Find broken links and orphan files across your entire project.",
    example: `$ kdb check
broken links:
  docs/old-guide.md:15 → setup.md (not found)
  src/lib.rs:3 → crate::legacy (no such module)

orphan files:
  docs/draft-notes.md (no inbound links)`,
  },
];

/* --------------- components --------------- */

function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <button
      onClick={handleCopy}
      className="inline-flex items-center gap-2 rounded-lg bg-(--accent) text-(--bg) px-5 py-2.5 text-sm font-medium transition-all hover:opacity-80 active:scale-[0.98] cursor-pointer"
    >
      {copied ? (
        <>
          <CheckIcon />
          Copied
        </>
      ) : (
        <>
          <ClipboardIcon />
          {label}
        </>
      )}
    </button>
  );
}

function CodeBlock({
  children,
  copyable,
}: {
  children: string;
  copyable?: boolean;
}) {
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    navigator.clipboard.writeText(children);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="relative group">
      <pre className="rounded-lg border border-(--code-border) bg-(--code-bg) p-4 text-sm leading-relaxed overflow-x-auto">
        <code>{children}</code>
      </pre>
      {copyable && (
        <button
          onClick={handleCopy}
          className="absolute top-3 right-3 opacity-0 group-hover:opacity-100 transition-opacity rounded-md border border-(--border) bg-(--code-bg) px-2 py-1 text-xs text-(--muted) hover:text-(--fg) cursor-pointer"
        >
          {copied ? "Copied" : "Copy"}
        </button>
      )}
    </div>
  );
}

function TerminalBlock({ children }: { children: string }) {
  return (
    <div className="rounded-lg border border-(--code-border) bg-(--code-bg) overflow-hidden">
      <div className="flex items-center gap-1.5 px-4 py-2.5 border-b border-(--code-border)">
        <div className="w-2.5 h-2.5 rounded-full bg-[#ff5f57]" />
        <div className="w-2.5 h-2.5 rounded-full bg-[#febc2e]" />
        <div className="w-2.5 h-2.5 rounded-full bg-[#28c840]" />
      </div>
      <pre className="p-4 text-sm leading-relaxed overflow-x-auto">
        <code>{children}</code>
      </pre>
    </div>
  );
}

/* --------------- page --------------- */

export default function Home() {
  return (
    <main className="mx-auto max-w-[720px] px-6 py-24 space-y-32">
      {/* ---- Hero ---- */}
      <section className="space-y-6 text-center">
        <h1 className="text-6xl font-bold tracking-tight sm:text-7xl">kdb</h1>
        <p className="text-xl text-(--fg) leading-relaxed sm:text-2xl">
          The fastest way for agents to navigate
          <br className="hidden sm:block" /> code + knowledge bases
        </p>
        <p className="text-(--muted) text-base max-w-lg mx-auto leading-relaxed">
          Built with Rust. Your agents explore code faster and burn fewer
          tokens. Fast, precise, no overhead.
        </p>
        <div className="flex items-center justify-center gap-3 pt-2">
          <CopyButton text={INSTALL_PROMPT} label="Copy install prompt" />
          <a
            href="https://github.com/dremnik/kdb"
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center justify-center rounded-lg border border-(--border) px-4 py-2.5 text-sm text-(--muted) transition-colors hover:text-(--fg) hover:border-(--muted)"
          >
            <GithubIcon />
          </a>
        </div>
      </section>

      {/* ---- Demo video ---- */}
      <section className="space-y-6">
        <h2 className="text-center text-lg font-medium text-(--muted)">
          See the difference
        </h2>
        <div className="aspect-video rounded-lg border border-(--code-border) bg-(--code-bg) flex items-center justify-center">
          <p className="text-(--muted) text-sm">
            Demo video — coming soon
          </p>
        </div>
        <p className="text-center text-sm text-(--muted)">
          Agent codebase exploration — with vs without kdb
        </p>
      </section>

      {/* ---- Agent setup ---- */}
      <section className="space-y-6">
        <div className="space-y-2 text-center">
          <h2 className="text-2xl font-semibold">What your agent gets</h2>
          <p className="text-(--muted) text-sm">
            The install prompt adds these instructions to your agent&apos;s
            system prompt.
          </p>
        </div>
        <CodeBlock copyable>{SYSTEM_PROMPT_BLOCK}</CodeBlock>
        <CodeBlock copyable>{CLI_HELP}</CodeBlock>
      </section>

      {/* ---- Usage ---- */}
      <section className="space-y-6">
        <h2 className="text-2xl font-semibold text-center">Usage</h2>
        <div className="space-y-4">
          <TerminalBlock>{EXAMPLE_SYMBOLS}</TerminalBlock>
          <TerminalBlock>{EXAMPLE_REFS}</TerminalBlock>
          <TerminalBlock>{EXAMPLE_TREE}</TerminalBlock>
        </div>
      </section>

      {/* ---- Features ---- */}
      <section className="space-y-12">
        <h2 className="text-2xl font-semibold text-center">Features</h2>
        {FEATURES.map((feat) => (
          <div key={feat.name} className="space-y-3">
            <div>
              <h3 className="text-lg font-medium">
                <code className="text-(--green)">kdb {feat.name}</code>
              </h3>
              <p className="text-(--muted) text-sm mt-1">{feat.desc}</p>
            </div>
            <TerminalBlock>{feat.example}</TerminalBlock>
          </div>
        ))}
      </section>

      {/* ---- Footer ---- */}
      <footer className="border-t border-(--border) pt-8 flex items-center justify-between text-sm text-(--muted)">
        <span>kdb</span>
        <a
          href="https://github.com/dremnik/kdb"
          target="_blank"
          rel="noopener noreferrer"
          className="hover:text-(--fg) transition-colors"
        >
          GitHub
        </a>
      </footer>
    </main>
  );
}
