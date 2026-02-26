use clap::{Parser, Subcommand};
use std::path::PathBuf;

// -----------------------
// src/main.rs
//
// struct Cli          L19
// enum Command        L25
// async fn main()    L123
// -----------------------

#[derive(Debug, Parser)]
#[command(
    name = "kdb",
    version,
    about = "Code intelligence CLI and LSP for knowledge bases",
    after_help = "pls report bugs: https://github.com/dremnik/kdb/issues"
)]
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
    /// Print symbols for files and/or directories.
    Symbols {
        /// File or directory paths to inspect (accepts multiple).
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Select symbols by name or `Parent::name` (single file only).
        #[arg(short = 's', long = "symbol", num_args = 1..)]
        symbols: Vec<String>,
        /// Emit structured JSON output.
        #[arg(long)]
        json: bool,
        /// Only include public/exported symbols.
        #[arg(long = "public")]
        public_only: bool,
    },
    /// Find inbound references to a markdown target or code symbol.
    Refs {
        /// Symbol target expression (e.g. `notes.md#getting-started`).
        target: String,
        /// Code symbol name for code reference mode.
        #[arg(short = 's', long = "symbol")]
        symbol: Option<String>,
        /// Show N lines of context around each symbol reference (text mode only).
        #[arg(short = 'c', long = "context")]
        context: Option<usize>,
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
            paths,
            symbols,
            json,
            public_only,
        } => kdb::cmd::symbols(paths, symbols, json, public_only),
        Command::Refs {
            target,
            symbol,
            context,
            json,
            count,
        } => kdb::cmd::refs(target, symbol, context, json, count),
        Command::Deps { target, json } => kdb::cmd::deps(target, json),
        Command::Graph { path } => kdb::cmd::graph(path),
        Command::Fmt { path } => kdb::cmd::format(path),
        Command::Lsp { path } => kdb::lsp::serve(path).await,
    };

    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
