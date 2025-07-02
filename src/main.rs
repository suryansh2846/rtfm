mod man_db;
mod trie;
mod tui;

use crate::man_db::ManDb;
use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio;

/// CLI for browsing man pages and tldr cheatsheets
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Manual section to use (default: 1)
    #[arg(short, long, default_value_t = 1)]
    section: u8,
}

/// Available subcommands
#[derive(Subcommand)]
enum Commands {
    /// List commands starting with prefix
    Getmans { prefix: String },
    /// Show man page for command
    Getman { command: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let man_db = ManDb::load(cli.section)?;

    match cli.command {
        Some(Commands::Getmans { prefix }) => {
            for word in man_db.commands_starting_with(&prefix) {
                println!("{}", word);
            }
        }
        Some(Commands::Getman { command }) => {
            man_db.display_man_page(&command)?;
        }
        None => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(tui::run_tui(man_db))?;
        }
    }

    Ok(())
}
