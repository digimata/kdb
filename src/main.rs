use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "kdb", version, about = "Markdown knowledge base CLI and LSP")]
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
        /// Optional starting path to discover kdb root from.
        path: Option<PathBuf>,
    },
    /// Print the heading outline for a markdown file.
    Outline {
        /// File path to outline.
        file: PathBuf,
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
        Command::Check { path } => match kdb::cmd::check(path) {
            Ok(has_issues) => {
                if has_issues {
                    std::process::exit(1);
                }
                Ok(())
            }
            Err(error) => Err(error),
        },
        Command::Outline { file } => kdb::cmd::outline(file),
        Command::Lsp { path } => kdb::cmd::lsp(path).await,
    };

    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
