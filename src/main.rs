use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use sensei::{book, App, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "sensei")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a book to your library
    Add {
        /// Path to EPUB file or markdown directory
        path: String,
    },
    /// List books in your library
    List,
    /// Remove a book from your library
    Remove {
        /// Name or ID of the book to remove
        name: String,
    },
    /// Export your progress and notes
    Export {
        /// Output path for CLAUDE.md
        #[arg(short, long, default_value = "CLAUDE.md")]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sensei=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Add { path }) => {
            let path = PathBuf::from(&path);
            println!("Adding book from: {}", path.display());

            match book::add_book(&path) {
                Ok(entry) => {
                    println!("Successfully added: {}", entry.metadata.title);
                    println!("  ID: {}", entry.metadata.id);
                    if let Some(author) = &entry.metadata.author {
                        println!("  Author: {}", author);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to add book: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::List) => {
            let library = book::Library::load()?;

            if library.list().is_empty() {
                println!("No books in library. Add one with: sensei add <path>");
            } else {
                println!("Books in library:");
                println!();
                for entry in library.list() {
                    println!("  {} - {}", entry.metadata.id, entry.metadata.title);
                    if let Some(author) = &entry.metadata.author {
                        println!("    Author: {}", author);
                    }
                }
            }
        }
        Some(Commands::Remove { name }) => {
            // Try to find by ID first, then by title
            let library = book::Library::load()?;

            let id_to_remove = if library.find_by_id(&name).is_some() {
                name.clone()
            } else if let Some(entry) = library.find_by_title(&name) {
                entry.metadata.id.clone()
            } else {
                eprintln!("Book not found: {}", name);
                std::process::exit(1);
            };

            match book::remove_book(&id_to_remove) {
                Ok(true) => {
                    println!("Removed: {}", id_to_remove);
                }
                Ok(false) => {
                    eprintln!("Book not found: {}", name);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to remove book: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Export { output }) => {
            // TODO: Implement full export
            println!("Export to {} - Coming soon!", output);
        }
        None => {
            // Launch TUI
            let config = Config::load()?;
            let mut app = App::new(config)?;
            app.run().await?;
        }
    }

    Ok(())
}
