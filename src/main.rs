mod trie;
mod tui;
mod man_db;

use clap::{Parser, Subcommand};
use anyhow::Result;
use crate::man_db::ManDb;
use tokio;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, default_value_t = 1)]
    manpage: u8,
}

#[derive(Subcommand)]
enum Commands {
    Getmans {
        prefix: String,
    },
    Getman {
        command: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let man_db = ManDb::load(cli.manpage)?;

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