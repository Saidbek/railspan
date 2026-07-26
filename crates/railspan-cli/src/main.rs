use clap::{Parser, Subcommand, ValueEnum};
use railspan_server::ServeConfig;
use std::net::SocketAddr;
use std::path::PathBuf;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "railspan", version, about = "Lightweight Rails-first APM")]
struct Cli {
    /// Log format: text (default) or json for production aggregators
    #[arg(
        long,
        env = "RAILSPAN_LOG_FORMAT",
        global = true,
        default_value = "text"
    )]
    log_format: LogFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run ingest + storage + UI in one process
    Serve {
        #[arg(long, env = "RAILSPAN_INGEST_ADDR", default_value = "127.0.0.1:7421")]
        addr: SocketAddr,
        #[arg(long, env = "RAILSPAN_DATA_DIR", default_value = "./data")]
        data_dir: PathBuf,
        /// Bearer token for POST /v1/* ingest
        #[arg(long, env = "RAILSPAN_API_KEY")]
        api_key: Option<String>,
        /// Bearer token for GET /api/* UI queries (defaults to --api-key)
        #[arg(long, env = "RAILSPAN_UI_TOKEN")]
        ui_token: Option<String>,
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
        /// Local app source root for UI code highlight (`GET /api/v1/source`)
        #[arg(long, env = "RAILSPAN_SOURCE_ROOT")]
        source_root: Option<PathBuf>,
        /// Dev-only: do not serve embedded Vue assets; redirect UI routes here
        /// (e.g. http://127.0.0.1:5173 for Vite). API routes stay on this process.
        #[arg(long, env = "RAILSPAN_DEV_UI_URL")]
        dev_ui_url: Option<String>,
    },
}

fn init_tracing(format: LogFormat) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tower_http=info".into());
    match format {
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_current_span(false)
                .with_span_list(false)
                .init();
        }
        LogFormat::Text => {
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.log_format);

    match cli.command {
        Commands::Serve {
            addr,
            data_dir,
            api_key,
            ui_token,
            sample_rate,
            slow_ms,
            retention_days,
            n1_threshold,
            source_root,
            dev_ui_url,
        } => {
            info!(%addr, data_dir = %data_dir.display(), "starting railspan serve");
            railspan_server::serve(ServeConfig {
                addr,
                data_dir,
                api_key,
                ui_token,
                sample_rate,
                slow_ms,
                retention_days,
                n1_threshold,
                source_root,
                dev_ui_url,
            })
            .await?;
        }
    }
    Ok(())
}
