use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "opc", about = "OPC - One Person Company CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the OPC server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3100")]
        port: u16,

        /// PostgreSQL port for embedded instance
        #[arg(long, default_value = "5433")]
        pg_port: u16,
    },
    /// Reset the embedded database
    Reset,
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, pg_port } => {
            std::env::set_var("PORT", port.to_string());
            std::env::set_var("PG_PORT", pg_port.to_string());
            println!("Starting OPC server on port {}...", port);
            println!("Use `cargo run -p opc-server` to start the server directly.");
        }
        Commands::Reset => {
            let db_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("db");
            if db_dir.exists() {
                std::fs::remove_dir_all(&db_dir)?;
                println!("Database reset. Removed {:?}", db_dir);
            } else {
                println!("No database directory found at {:?}", db_dir);
            }
        }
        Commands::Version => {
            println!("opc {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
