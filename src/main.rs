use anyhow::Result;
use clap::{Parser, Subcommand};
use sensei::{App, Config};
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
        /// Name of the book to remove
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
            todo!("Implement book addition: {}", path);
        }
        Some(Commands::List) => {
            todo!("Implement book listing");
        }
        Some(Commands::Remove { name }) => {
            todo!("Implement book removal: {}", name);
        }
        Some(Commands::Export { output }) => {
            todo!("Implement export to: {}", output);
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
