use clap::{Parser, Subcommand};
use std::path::PathBuf;

// -----------------------
// src/main.rs
//
// struct Cli          L14
// enum Command        L20
// async fn main()    L116
// -----------------------

#[derive(Debug, Parser)]
#[command(name = "kdb", version, about = "Code intelligence CLI and LSP for knowledge bases")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize a kdb project in a directory.
    Init {
        /// Optional directory path (defaults to current directory).
        path: Option<PathBuf>,
    },
    /// Report broken links and orphan files.
    Check {
        /// Print each orphan file path.
        #[arg(long)]
        orphans: bool,
        /// Optional file or directory path to scope check output to.
        path: Option<PathBuf>,
    },
    /// Print the heading outline for a markdown file.
    Outline {
        /// File path to outline.
        file: PathBuf,
    },
    /// Print a filtered directory tree for the project.
    Tree {
        /// Maximum display depth (same as `tree -L`).
        #[arg(short = 'L', long = "level", alias = "depth")]
        level: Option<usize>,
        /// Include hidden entries (same as `tree -a`).
        #[arg(short = 'a', long)]
        all: bool,
        /// Show directories only (same as `tree -d`).
        #[arg(short = 'd', long = "dirs-only")]
        dirs_only: bool,
        /// Print full relative paths for each entry (same as `tree -f`).
        #[arg(short = 'f', long = "full-path")]
        full_path: bool,
        /// Exclude entries matching a wildcard pattern (same as `tree -I`).
        #[arg(short = 'I', long = "ignore")]
        ignore: Vec<String>,
        /// Include only entries matching a wildcard pattern (same as `tree -P`).
        #[arg(short = 'P', long = "pattern")]
        pattern: Vec<String>,
        /// Emit machine-readable JSON output.
        #[arg(short = 'J', long)]
        json: bool,
        /// Optional path to render (defaults to project root).
        path: Option<PathBuf>,
    },
    /// Print symbols for a markdown or supported code file.
    Symbols {
        /// File path to inspect.
        path: PathBuf,
        /// Select a specific symbol by name or `Parent::name`.
        #[arg(short = 's', long = "symbol")]
        symbol: Option<String>,
        /// Emit structured JSON output.
        #[arg(long)]
        json: bool,
        /// Only include public/exported symbols.
        #[arg(long = "public")]
        public_only: bool,
    },
    /// Find inbound references to a markdown file or heading.
    Refs {
        /// Symbol target expression (e.g. `notes.md#getting-started`).
        target: String,
        /// Emit structured JSON output.
        #[arg(long)]
        json: bool,
        /// Print only the number of inbound references.
        #[arg(long)]
        count: bool,
    },
    /// Print direct dependencies for a file/symbol target.
    Deps {
        /// File or symbol target expression.
        target: String,
        /// Emit structured JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Render a dependency graph for markdown and code symbols.
    Graph {
        /// Optional starting path to discover kdb root from.
        path: Option<PathBuf>,
    },
    /// Generate or update code index headers in supported code files.
    Fmt {
        /// Optional file or directory path to format (defaults to project root).
        path: Option<PathBuf>,
    },
    /// Run the language server over stdio.
    Lsp {
        /// Optional starting path to discover kdb root from.
        path: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init { path } => kdb::cmd::init(path),
        Command::Check { path, orphans } => match kdb::cmd::check(path, orphans) {
            Ok(has_issues) => {
                if has_issues {
                    std::process::exit(1);
                }
                Ok(())
            }
            Err(error) => Err(error),
        },
        Command::Outline { file } => kdb::cmd::outline(file),
        Command::Tree {
            level,
            all,
            dirs_only,
            full_path,
            ignore,
            pattern,
            json,
            path,
        } => kdb::cmd::tree(
            path, level, json, all, dirs_only, full_path, ignore, pattern,
        ),
        Command::Symbols {
            path,
            symbol,
            json,
            public_only,
        } => kdb::cmd::symbols(path, symbol, json, public_only),
        Command::Refs {
            target,
            json,
            count,
        } => kdb::cmd::refs(target, json, count),
        Command::Deps { target, json } => kdb::cmd::deps(target, json),
        Command::Graph { path } => kdb::cmd::graph(path),
        Command::Fmt { path } => kdb::cmd::fmt(path),
        Command::Lsp { path } => kdb::lsp::serve(path).await,
    };

    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
