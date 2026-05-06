use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::ExecutableCommand;
use std::io;

use mmbak_parser::{MMBakFile, Result};

#[derive(Parser)]
#[command(name = "mmbak")]
#[command(about = "MoneyManager backup file viewer")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Backup file to open (launches TUI)
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync account balances to Google Sheets Entity_Log
    SyncSheet {
        /// Path to sync configuration TOML
        #[arg(short, long, default_value = "sync_config.toml")]
        config: PathBuf,

        /// Backup file to read
        file: PathBuf,

        /// Preview what would be updated without writing
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();

    let result = match (cli.command, cli.file) {
        (Some(Commands::SyncSheet { config, file, dry_run }), _) => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(sync_sheet(file, config, dry_run))
        }
        (None, Some(file)) => run_tui(file),
        (None, None) => {
            eprintln!("Error: No file specified.\n\nUsage: mmbak <FILE>\n       mmbak sync-sheet --config <CFG> <FILE> [--dry-run]");
            process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn run_tui(path: PathBuf) -> Result<()> {
    let backup = MMBakFile::open(&path)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;

    let mut terminal = ratatui::init();
    let mut app = mmbak_parser::tui::App::new(backup);
    let result = app.run(&mut terminal);

    ratatui::restore();
    stdout.execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    result
}

async fn sync_sheet(file: PathBuf, config: PathBuf, dry_run: bool) -> Result<()> {
    use colored::Colorize;

    let backup = MMBakFile::open(&file)?;
    let sync_config = mmbak_parser::SyncConfig::from_file(&config)?;

    if dry_run {
        println!("{}", "=== DRY RUN ===".yellow().bold());
    }

    mmbak_parser::sync_balances(&backup, &sync_config, dry_run).await
}
