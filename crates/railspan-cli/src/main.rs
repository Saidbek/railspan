use clap::{Parser, Subcommand};
use railspan_agent::{serve, AgentMetrics, AgentState};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "railspan", version, about = "Lightweight Rails-first APM")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run agent ingest (and later UI/server in one process)
    Serve {
        /// Ingest listen address
        #[arg(long, env = "RAILSPAN_INGEST_ADDR", default_value = "127.0.0.1:7421")]
        addr: SocketAddr,
        /// Optional API key for ingest (Bearer)
        #[arg(long, env = "RAILSPAN_API_KEY")]
        api_key: Option<String>,
    },
    /// Agent-only mode (alias of serve for now)
    Agent {
        #[arg(long, env = "RAILSPAN_INGEST_ADDR", default_value = "127.0.0.1:7421")]
        addr: SocketAddr,
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
        Commands::Serve { addr, api_key } | Commands::Agent { addr, api_key } => {
            info!(%addr, "starting railspan serve (agent ingest)");
            let _ = railspan_server::placeholder();
            let state = AgentState {
                api_key,
                metrics: Arc::new(AgentMetrics::default()),
            };
            serve(addr, state).await?;
        }
    }
    Ok(())
}
