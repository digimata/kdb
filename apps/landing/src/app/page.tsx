import { GithubIcon } from "@/components/ui/icons";
import { InstallBlock } from "@/components/install-block";
import { Terminal } from "@/components/terminal";
import { Footer } from "@/components/footer";
import { highlight } from "@/lib/highlight";
import type { CSSProperties } from "react";

// --------------------------------------------
// projects/kdb/apps/landing/src/app/page.tsx
//
// const EXAMPLE_OUTLINE                    L33
// const EXAMPLE_REFS                       L42
// const EXAMPLE_CHECK                      L47
// const EXAMPLE_RENDER                     L58
// const EXAMPLE_TREE                       L70
// const EXAMPLE_TASKS                      L96
// const EXAMPLE_TASK_RENDER               L107
// const PRINCIPLES                        L118
// const TASK_FEATURES                     L131
// const KB_FEATURES                       L149
// const editorialHeading                  L167
// const editorialHeadingSm                L174
// function Separator()                    L181
// function Header()                       L185
// function FeatureList()                  L208
// description                             L212
// items                                   L212
// title                                   L212
// marker                                  L213
// export default async function Home()    L238
// --------------------------------------------

const EXAMPLE_OUTLINE = `$ kdb outline notes/architecture.md
notes/architecture.md
  # Architecture          L1
  ## Overview              L5
  ## Components            L22
    ### Ingest             L24
    ### Store              L40
  ## Data flow             L68`;

const EXAMPLE_REFS = `$ kdb refs notes/architecture.md#components
notes/index.md:12         [architecture › components](architecture.md#components)
notes/onboarding.md:34    [[architecture#components]]
SOP/release.md:8          ![[architecture#components]]`;

const EXAMPLE_CHECK = `$ kdb check
broken links:
  notes/old-plan.md:15 → roadmap.md (not found)
  SOP/release.md:42   → architecture.md#deploy (no such heading)

broken embeds:
  notes/index.md:3    ![[glossary#obsolete-term]]

orphan files:
  notes/draft-2024.md (no inbound links)`;

const EXAMPLE_RENDER = `$ kdb render SOP/release.md
# Release SOP

## Preflight
- Verify architecture notes
  (inlined from architecture.md#components)
- ...

## Deploy
  (inlined from deploy.md#steps)
- ...`;

const EXAMPLE_TREE = `workspace/
├── .kdb/
│   └── index.db                  # SQLite: projects, tasks, cycles, labels
├── .cycles/
│   ├── index.md                  # rollup of every cycle
│   ├── C-14.md                   # active
│   └── C-13.md
├── projects/
│   ├── kdb/
│   │   ├── .tasks/
│   │   │   ├── index.md
│   │   │   ├── T-0120.md
│   │   │   └── T-0121.md
│   │   ├── notes/
│   │   │   └── architecture.md
│   │   └── README.md
│   ├── project-b/
│   │   └── .tasks/
│   │       └── T-0045.md
│   └── project-c/
│       └── docs/
│           └── roadmap.md
├── SOP/
│   └── release.md
└── README.md`;

const EXAMPLE_TASKS = `$ kdb projects add --slug hermaeus --alias HRM --path projects/hermaeus
registered: hermaeus (HRM)

$ kdb tasks add --project hermaeus "Wire source-link hover cards" -p 2 -c C-14
HRM-0121 added

$ kdb tasks list --status in_progress,open -n 3
HRM-0119  in_progress  p1  C-14  Deploy Archil mount
HRM-0120  open         p1  C-14  Entity resolver backfill
HRM-0121  open         p2  C-14  Wire source-link hover cards`;

const EXAMPLE_TASK_RENDER = `$ kdb render --project hermaeus --limit 10
projects/hermaeus/.tasks/index.md
projects/hermaeus/.tasks/T-0119.md
projects/hermaeus/.tasks/T-0120.md
projects/hermaeus/.tasks/T-0121.md

$ kdb cycles list
C-14  active   2026-04-20  2026-04-27
C-13  done     2026-04-13  2026-04-20
C-12  done     2026-04-06  2026-04-13`;

const PRINCIPLES = [
  {
    title: "Single source of truth.",
    description:
      "Notes, tasks, projects, cycles — one repository, one database. No tool-switching. No wondering where a decision was written down. No sync lag between Jira, Notion, and your editor.",
  },
  {
    title: "Context consolidation is imperative.",
    description:
      "Agents need consolidated context to operate effectively. Your notes, tasks, and plan have to sit on the same surface an agent can read.",
  },
];

const TASK_FEATURES = [
  {
    title: "Projects",
    description:
      "Register every project with a slug and a 3-letter alias. Tasks inherit stable IDs like HRM-0120 — same shape across every repo you work in.",
  },
  {
    title: "Tasks",
    description:
      "Priorities, statuses, cycles, labels. One row in SQLite, one T-NNNN.md file on disk. Your editor is the UI. Git is the sync.",
  },
  {
    title: "Cycles",
    description:
      "Week-long sprints with start and end dates. Planned, active, done, abandoned — nothing more. Scope the work, ship it, close the loop.",
  },
];

const KB_FEATURES = [
  {
    title: "One graph",
    description:
      "Headings are nodes. Links are edges. Every markdown file in your repo is parsed, every link resolved — no indexing server, no cloud, no seat fee.",
  },
  {
    title: "Broken links, found.",
    description:
      "kdb check reports broken links, broken embeds, and orphan files across the project. Wire it into CI and stop shipping rot into your own docs.",
  },
  {
    title: "Transclusion that works.",
    description:
      "Compose documents from canonical sources with ![[file#heading]]. kdb render resolves embeds recursively and prints to stdout.",
  },
];

const editorialHeading: CSSProperties = {
  fontSize: "clamp(28px, 4vw, 40px)",
  lineHeight: "1.15",
  letterSpacing: "-0.02em",
  fontWeight: 400,
};

const editorialHeadingSm: CSSProperties = {
  fontSize: "clamp(22px, 2.8vw, 30px)",
  lineHeight: "1.2",
  letterSpacing: "-0.02em",
  fontWeight: 400,
};

function Separator() {
  return <hr className="w-full border-0 border-t border-ds-gray-300" />;
}

function Header() {
  return (
    <header className="fixed top-0 right-0 left-0 z-50 flex items-center justify-between px-6 py-5 md:px-12 md:py-6">
      <a
        href="/"
        className="text-[22px] leading-none text-ds-gray-1000 select-none"
        aria-label="digimata"
      >
        Ξ
      </a>
      <a
        href="https://github.com/dremnik/kdb"
        target="_blank"
        rel="noopener noreferrer"
        className="text-ds-gray-1000 transition-opacity hover:opacity-70"
        aria-label="GitHub"
      >
        <GithubIcon />
      </a>
    </header>
  );
}

function FeatureList({
  items,
  marker = "numeric",
}: {
  items: { title: string; description: string }[];
  marker?: "numeric" | "paren";
}) {
  return (
    <ol className="flex flex-col gap-8">
      {items.map((feature, index) => (
        <li key={feature.title} className="flex gap-6">
          <span className="w-8 shrink-0 pt-1 font-mono text-sm text-ds-steel-500">
            {marker === "paren"
              ? `(${index + 1})`
              : String(index + 1).padStart(2, "0")}
          </span>
          <div className="flex flex-col gap-2">
            <h3 className="text-heading-16 text-ds-gray-1000">
              {feature.title}
            </h3>
            <p className="max-w-2xl text-copy-14 text-ds-gray-600">
              {feature.description}
            </p>
          </div>
        </li>
      ))}
    </ol>
  );
}

export default async function Home() {
  const [
    treeHtml,
    outlineHtml,
    refsHtml,
    checkHtml,
    renderHtml,
    tasksHtml,
    taskRenderHtml,
  ] = await Promise.all([
    highlight(EXAMPLE_TREE),
    highlight(EXAMPLE_OUTLINE),
    highlight(EXAMPLE_REFS),
    highlight(EXAMPLE_CHECK),
    highlight(EXAMPLE_RENDER),
    highlight(EXAMPLE_TASKS),
    highlight(EXAMPLE_TASK_RENDER),
  ]);

  const wordmarkStyle: CSSProperties = {
    ["--shiki-light" as string]: "#067A6E",
    ["--shiki-dark" as string]: "#50E3C2",
    color: "var(--shiki-dark)",
    fontFamily: "var(--font-mono)",
    fontSize: "clamp(56px, 8vw, 96px)",
    lineHeight: 1,
    fontWeight: 300,
    letterSpacing: "-0.04em",
  };

  return (
    <div className="min-h-screen">
      <Header />

      <main className="mx-auto flex w-full min-w-0 max-w-2xl flex-col gap-12 px-6 pt-32 pb-24 lg:max-w-3xl">
        {/* Hero — headline + manifesto */}
        <section className="flex flex-col items-start gap-8">
          <h1
            className="max-w-2xl animate-blur-rise text-ds-gray-1000"
            style={editorialHeading}
          >
            Your context should all live in a single knowledge repository{" "}
            <span className="text-ds-gray-500">— including your work.</span>
          </h1>

          <div className="flex flex-col gap-5 pt-4">
            <blockquote className="max-w-2xl animate-blur-rise animate-delay-200 border-l-2 border-ds-steel-500 pl-5 text-copy-15 text-ds-prose italic">
              If your data is stored in a database that a company can freely
              read and access — i.e. not end-to-end encrypted — the company
              will eventually update their ToS so they can use your data for
              AI training. The incentives are too strong to resist.
              <footer className="mt-3 text-copy-13 text-ds-gray-600 not-italic">
                —{" "}
                <a
                  href="https://x.com/kepano/status/1688610782509211648?s=20"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-ds-steel-500 transition-opacity hover:opacity-70"
                >
                  kepano
                </a>
              </footer>
            </blockquote>

            <p className="max-w-2xl animate-blur-rise animate-delay-300 text-copy-15 text-ds-gray-600">
              kdb is a CLI. Your database is a SQLite file on disk. Your
              tasks are markdown files in your repo. There is no{" "}
              <span className="font-mono text-ds-gray-1000">kdb</span> server
              with your work on it. There is no ToS to update. Delete the
              binary tomorrow — your data is exactly where you left it.
            </p>
          </div>
        </section>

        {/* Installation */}
        <section className="animate-blur-rise animate-delay-300">
          <InstallBlock />
        </section>

        <Separator />

        {/* Principles */}
        <section className="flex flex-col gap-8">
          <div className="flex flex-col gap-3">
            <h2 className="text-ds-gray-1000" style={editorialHeadingSm}>
              Principles.
            </h2>
            <p className="max-w-2xl text-copy-15 text-ds-gray-600">
              These are the things kdb is opinionated about.
            </p>
          </div>
          <FeatureList items={PRINCIPLES} marker="paren" />
        </section>

        <Separator />

        {/* Filesystem overview — proof of the ownership thesis */}
        <section className="flex flex-col gap-6">
          <div className="flex flex-col gap-3">
            <h2 className="text-ds-gray-1000" style={editorialHeadingSm}>
              On disk.{" "}
              <span className="text-ds-gray-500">Just files.</span>
            </h2>
            <p className="max-w-2xl text-copy-15 text-ds-gray-600">
              A kdb workspace is just a folder on disk. Every byte kdb
              writes into it is visible to you: a SQLite index in{" "}
              <code className="font-mono text-ds-gray-1000">.kdb/</code>,
              cycle files in{" "}
              <code className="font-mono text-ds-gray-1000">.cycles/</code>,
              materialized tasks in each project&apos;s{" "}
              <code className="font-mono text-ds-gray-1000">.tasks/</code>,
              and your own notes wherever you put them.
            </p>
          </div>
          <Terminal html={treeHtml} chrome={false} />
        </section>

        <Separator />

        {/* Tasks / projects */}
        <section className="flex flex-col gap-10">
          <div className="flex flex-col gap-3">
            <h2 className="text-ds-gray-1000" style={editorialHeadingSm}>
              Your plan.{" "}
              <span className="text-ds-gray-500">In the same graph.</span>
            </h2>
            <p className="max-w-2xl text-copy-15 text-ds-gray-600">
              Tasks aren&apos;t a separate product. They&apos;re nodes in the
              same graph as your notes. Projects get stable aliases, tasks
              materialize as markdown files in{" "}
              <code className="font-mono text-ds-gray-1000">.tasks/</code>,
              cycles keep you honest about scope. A CRUD app with a very small
              number of customers: you, and you again.
            </p>
          </div>
          <FeatureList items={TASK_FEATURES} />
        </section>

        {/* Tasks usage terminals */}
        <section className="flex min-w-0 flex-col gap-8 pt-2">
          <div className="flex min-w-0 flex-col gap-3">
            <p className="text-copy-14 text-ds-gray-600">
              <span className="font-mono text-ds-gray-1000">
                kdb projects / kdb tasks
              </span>{" "}
              — register projects, open tasks, set priorities and cycles.
            </p>
            <Terminal html={tasksHtml} />
          </div>

          <div className="flex min-w-0 flex-col gap-3">
            <p className="text-copy-14 text-ds-gray-600">
              <span className="font-mono text-ds-gray-1000">
                kdb render --project
              </span>{" "}
              materializes{" "}
              <code className="font-mono text-ds-gray-1000">index.md</code> and
              one file per active task into{" "}
              <code className="font-mono text-ds-gray-1000">.tasks/</code>.
              Commit them. Diff them. Review them.
            </p>
            <Terminal html={taskRenderHtml} />
          </div>
        </section>

        <Separator />

        {/* Knowledge-base features */}
        <section className="flex flex-col gap-10">
          <div className="flex flex-col gap-3">
            <h2 className="text-ds-gray-1000" style={editorialHeadingSm}>
              Your notes.{" "}
              <span className="text-ds-gray-500">Your graph.</span>
            </h2>
            <p className="max-w-2xl text-copy-15 text-ds-gray-600">
              Every markdown file in your repo is a node. Every link is an
              edge. kdb parses it all, resolves every reference, and hands the
              graph back — from the CLI, or from any editor over LSP.
            </p>
          </div>
          <FeatureList items={KB_FEATURES} />
        </section>

        {/* KB usage terminals */}
        <section className="flex min-w-0 flex-col gap-8 pt-2">
          <div className="flex min-w-0 flex-col gap-3">
            <p className="text-copy-14 text-ds-gray-600">
              <span className="font-mono text-ds-gray-1000">kdb outline</span>{" "}
              — print the outline (headings) of any markdown file.
            </p>
            <Terminal html={outlineHtml} />
          </div>

          <div className="flex min-w-0 flex-col gap-3">
            <p className="text-copy-14 text-ds-gray-600">
              <span className="font-mono text-ds-gray-1000">kdb refs</span> —
              find every file that links to a page or heading.
            </p>
            <Terminal html={refsHtml} />
          </div>

          <div className="flex min-w-0 flex-col gap-3">
            <p className="text-copy-14 text-ds-gray-600">
              <span className="font-mono text-ds-gray-1000">kdb check</span> —
              broken links, broken embeds, and orphan files across the project.
            </p>
            <Terminal html={checkHtml} />
          </div>

          <div className="flex min-w-0 flex-col gap-3">
            <p className="text-copy-14 text-ds-gray-600">
              <span className="font-mono text-ds-gray-1000">kdb render</span> —
              resolve{" "}
              <code className="font-mono text-ds-gray-1000">![[]]</code>{" "}
              embeds recursively to stdout.
            </p>
            <Terminal html={renderHtml} />
          </div>
        </section>

      </main>

      <Footer />
    </div>
  );
}
