use clap::Parser;
use miette::{IntoDiagnostic, miette};
use mq_tui::{App, ThemeName};
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mq_tui")]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "TUI for mq, a jq-like Markdown processing tool", long_about = None)]
#[command(after_help = "Examples:\n\n
    Open a Markdown file:\n
    $ mq_tui README.md\n\n
    Open multiple Markdown files as tabs:\n
    $ mq_tui README.md CHANGELOG.md\n\n
    Watch a file and reload automatically when it changes:\n
    $ mq_tui --watch README.md\n\n
    Read from stdin:\n
    $ cat README.md | mq_tui\n\n
    Use with mq CLI:\n
    $ mq tui file.md")]
struct Cli {
    /// Paths to the Markdown files to open (each opens as a tab)
    #[arg(value_name = "FILE")]
    file_paths: Vec<PathBuf>,

    /// Watch opened files and reload automatically when they change on disk
    #[arg(short, long)]
    watch: bool,

    /// Color theme (overrides config.toml for this run)
    #[arg(long, value_name = "THEME")]
    theme: Option<ThemeName>,

    /// Hide the persistent key-hint bar (overrides config.toml for this run)
    #[arg(long)]
    no_hints: bool,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    let mut app = if !cli.file_paths.is_empty() {
        let mut files = Vec::with_capacity(cli.file_paths.len());
        for file_path in &cli.file_paths {
            let content = fs::read_to_string(file_path).into_diagnostic()?;
            let filename = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file.md")
                .to_string();
            files.push((content, filename, file_path.clone()));
        }
        App::with_files_with_paths(files)
    } else if !io::stdin().is_terminal() {
        if cli.watch {
            return Err(miette!("--watch cannot be used when reading from stdin"));
        }
        let mut content = String::new();
        io::stdin().read_to_string(&mut content).into_diagnostic()?;
        App::with_file(content, "stdin".to_string())
    } else {
        return Err(miette!(
            "No file path provided.\nUsage: mq_tui <FILE>...\nFor more information, try '--help'"
        ));
    };

    if cli.watch {
        app.set_watch(true);
    }
    if let Some(theme) = cli.theme {
        app.set_theme_name(theme);
    }
    if cli.no_hints {
        app.set_show_hint_bar(false);
    }

    app.run()?;

    Ok(())
}
