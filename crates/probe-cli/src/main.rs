//! XCProbe CLI - Non-intrusive system discovery and collection tool.

mod bundle;
mod collector;
mod commands;
mod executor;
mod pack;
mod parsers;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use xcprobe_common::OsType;

#[derive(Parser)]
#[command(name = "probe-cli")]
#[command(
    author,
    version,
    about = "Non-intrusive system discovery and collection tool"
)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output format (json, text)
    #[arg(long, global = true, default_value = "text")]
    format: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect system information from a target
    Collect {
        /// Target host (hostname or IP address)
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

        /// SSH password (not recommended, use key-based auth)
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

    /// Validate a collection bundle
    ValidateBundle {
        /// Input bundle file path
        #[arg(long, short = 'i')]
        r#in: PathBuf,

        /// Check evidence references
        #[arg(long)]
        check_evidence: bool,

        /// Verify checksums
        #[arg(long)]
        verify_checksums: bool,
    },

    /// Generate a pack plan from a bundle
    PlanPack {
        /// Input bundle file path
        #[arg(long)]
        bundle: PathBuf,

        /// Output pack plan file path
        #[arg(long, short)]
        out: PathBuf,
    },

    /// Execute pack plan to collect files from target
    Pack {
        /// Target host
        #[arg(long)]
        target: String,

        /// Target operating system
        #[arg(long)]
        os: String,

        /// Pack plan file path
        #[arg(long)]
        plan: PathBuf,

        /// Output directory for collected files
        #[arg(long, short)]
        out: PathBuf,

        /// SSH/WinRM connection options (same as collect)
        #[arg(long, default_value = "22")]
        ssh_port: u16,

        #[arg(long)]
        ssh_user: Option<String>,

        #[arg(long)]
        ssh_key: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
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
            info!("Starting collection from {} ({:?})", target, os_type);

            let config = collector::CollectorConfig {
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

            let collector = collector::Collector::new(config)?;
            let bundle = collector.collect().await?;

            bundle::write_bundle(&bundle, &out)?;
            info!("Bundle written to {:?}", out);
        }

        Commands::ValidateBundle {
            r#in: input,
            check_evidence,
            verify_checksums,
        } => {
            info!("Validating bundle: {:?}", input);
            let result = bundle::validate_bundle_file(&input, check_evidence, verify_checksums)?;

            if result.valid {
                println!("Bundle is valid");
            } else {
                println!("Bundle validation failed:");
                for error in &result.errors {
                    println!("  - {}", error);
                }
            }

            if !result.warnings.is_empty() {
                println!("Warnings:");
                for warning in &result.warnings {
                    println!("  - {}", warning);
                }
            }
        }

        Commands::PlanPack { bundle, out } => {
            info!("Generating pack plan from: {:?}", bundle);
            let bundle_data = bundle::read_bundle(&bundle)?;
            let plan = pack::generate_pack_plan(&bundle_data)?;

            let json = serde_json::to_string_pretty(&plan)?;
            std::fs::write(&out, json)?;
            info!("Pack plan written to {:?}", out);
        }

        Commands::Pack {
            target,
            os,
            plan,
            out,
            ssh_port,
            ssh_user,
            ssh_key,
        } => {
            let os_type: OsType = os.parse()?;
            info!("Executing pack plan on {} ({:?})", target, os_type);

            let plan_content = std::fs::read_to_string(&plan)?;
            let pack_plan: xcprobe_bundle_schema::PackPlan = serde_json::from_str(&plan_content)?;

            pack::execute_pack(
                &pack_plan, &target, os_type, ssh_port, ssh_user, ssh_key, &out,
            )
            .await?;
            info!("Pack completed to {:?}", out);
        }
    }

    Ok(())
}
