use clap::{Parser, Subcommand};
use railspan_server::ServeConfig;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "railspan", version, about = "Lightweight Rails-first APM")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run ingest + storage + UI in one process
    Serve {
        /// Listen address (ingest, API, UI)
        #[arg(long, env = "RAILSPAN_INGEST_ADDR", default_value = "127.0.0.1:7421")]
        addr: SocketAddr,
        /// Directory for SQLite data
        #[arg(long, env = "RAILSPAN_DATA_DIR", default_value = "./data")]
        data_dir: PathBuf,
        /// Optional API key for ingest (Bearer)
        #[arg(long, env = "RAILSPAN_API_KEY")]
        api_key: Option<String>,
    },
    /// Alias of serve
    Agent {
        #[arg(long, env = "RAILSPAN_INGEST_ADDR", default_value = "127.0.0.1:7421")]
        addr: SocketAddr,
        #[arg(long, env = "RAILSPAN_DATA_DIR", default_value = "./data")]
        data_dir: PathBuf,
        #[arg(long, env = "RAILSPAN_API_KEY")]
        api_key: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Serve {
            addr,
            data_dir,
            api_key,
        }
        | Commands::Agent {
            addr,
            data_dir,
            api_key,
        } => {
            info!(%addr, data_dir = %data_dir.display(), "starting railspan serve");
            railspan_server::serve(ServeConfig {
                addr,
                data_dir,
                api_key,
            })
            .await?;
        }
    }
    Ok(())
}
