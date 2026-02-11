//! XCProbe - System discovery, collection and containerization tool.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use xcprobe_common::OsType;

#[derive(Parser)]
#[command(name = "xcprobe")]
#[command(
    author,
    version,
    about = "Discover running services on a host and generate Docker artifacts"
)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect system information from a target host
    Collect {
        /// Target host (hostname or IP, or "localhost" for local collection)
        #[arg(long)]
        target: String,

        /// Target operating system (linux, windows)
        #[arg(long)]
        os: String,

        /// Collection mode (remote, local-ephemeral)
        #[arg(long, default_value = "remote")]
        mode: String,

        /// Output bundle file path
        #[arg(long, short)]
        out: PathBuf,

        /// SSH port (for remote Linux)
        #[arg(long, default_value = "22")]
        ssh_port: u16,

        /// SSH user
        #[arg(long)]
        ssh_user: Option<String>,

        /// SSH private key path
        #[arg(long)]
        ssh_key: Option<PathBuf>,

        /// SSH password
        #[arg(long)]
        ssh_password: Option<String>,

        /// WinRM port (for remote Windows)
        #[arg(long, default_value = "5985")]
        winrm_port: u16,

        /// WinRM user
        #[arg(long)]
        winrm_user: Option<String>,

        /// WinRM password
        #[arg(long)]
        winrm_password: Option<String>,

        /// Use HTTPS for WinRM
        #[arg(long)]
        winrm_https: bool,

        /// Collection timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,
    },

    /// Analyze a bundle and generate Docker artifacts
    Analyze {
        /// Input bundle file path
        #[arg(long)]
        bundle: PathBuf,

        /// Output directory for artifacts
        #[arg(long, short)]
        out: PathBuf,

        /// Cluster name prefix
        #[arg(long, default_value = "app")]
        cluster_prefix: String,

        /// Minimum confidence threshold (0.0-1.0)
        #[arg(long, default_value = "0.7")]
        min_confidence: f64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false))
        .with(filter)
        .init();

    match cli.command {
        Commands::Collect {
            target,
            os,
            mode,
            out,
            ssh_port,
            ssh_user,
            ssh_key,
            ssh_password,
            winrm_port,
            winrm_user,
            winrm_password,
            winrm_https,
            timeout,
        } => {
            let os_type: OsType = os.parse()?;
            info!("Collecting from {} ({:?})", target, os_type);

            let config = xcprobe_collector::collector::CollectorConfig {
                target,
                os_type,
                mode: mode.parse()?,
                ssh_port,
                ssh_user,
                ssh_key,
                ssh_password,
                winrm_port,
                winrm_user,
                winrm_password,
                winrm_https,
                timeout_seconds: timeout,
            };

            let collector = xcprobe_collector::collector::Collector::new(config)?;
            let bundle = collector.collect().await?;

            xcprobe_collector::bundle::write_bundle(&bundle, &out)?;
            info!("Bundle written to {:?}", out);
        }

        Commands::Analyze {
            bundle,
            out,
            cluster_prefix,
            min_confidence,
        } => {
            info!("Analyzing bundle: {:?}", bundle);

            let bundle_data = xcprobe_collector::bundle::read_bundle(&bundle)?;

            let pack_plan =
                xcprobe_analyzer::analyze_bundle(&bundle_data, &cluster_prefix, min_confidence)?;

            std::fs::create_dir_all(&out)?;
            xcprobe_analyzer::generate_artifacts(&pack_plan, &out)?;

            let plan_path = out.join("packplan.json");
            let plan_json = serde_json::to_string_pretty(&pack_plan)?;
            std::fs::write(&plan_path, plan_json)?;

            info!("Analysis complete. Artifacts written to {:?}", out);
        }
    }

    Ok(())
}
