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
        #[arg(long, env = "RAILSPAN_INGEST_ADDR", default_value = "127.0.0.1:7421")]
        addr: SocketAddr,
        #[arg(long, env = "RAILSPAN_DATA_DIR", default_value = "./data")]
        data_dir: PathBuf,
        #[arg(long, env = "RAILSPAN_API_KEY")]
        api_key: Option<String>,
        /// Keep probability for non-error/non-slow traces (0.0–1.0)
        #[arg(long, env = "RAILSPAN_SAMPLE_RATE", default_value = "1.0")]
        sample_rate: f64,
        /// Always keep roots slower than this (ms)
        #[arg(long, env = "RAILSPAN_SLOW_MS", default_value = "500")]
        slow_ms: u64,
        /// Delete traces older than N days
        #[arg(long, env = "RAILSPAN_RETENTION_DAYS", default_value = "7")]
        retention_days: u64,
        /// N+1 detection threshold (identical SQL count)
        #[arg(long, env = "RAILSPAN_N1_THRESHOLD", default_value = "5")]
        n1_threshold: u32,
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
            sample_rate,
            slow_ms,
            retention_days,
            n1_threshold,
        } => {
            info!(%addr, data_dir = %data_dir.display(), "starting railspan serve");
            railspan_server::serve(ServeConfig {
                addr,
                data_dir,
                api_key,
                sample_rate,
                slow_ms,
                retention_days,
                n1_threshold,
            })
            .await?;
        }
    }
    Ok(())
}
