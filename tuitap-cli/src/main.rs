use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;

use tuitap_gateway::{Gateway, GatewayConfig};

mod attach;
mod config;
mod run;

use config::CliConfig;

// ── CLI definition ────────────────────────────────────────────────────────────

/// TUITap — cross-platform bidirectional terminal gateway.
#[derive(Parser, Debug)]
#[command(
    name = "tuitap",
    version,
    about = "Transparently wrap or attach to any TUI process and bridge its output to push channels"
)]
struct Cli {
    /// Path to configuration file (default: ./tuitap.toml).
    #[arg(short, long, default_value = "tuitap.toml")]
    config: std::path::PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Wrap a new process under a PTY and start the gateway.
    ///
    /// Example:
    ///   tuitap run -- bash
    ///   tuitap run -- htop
    Run {
        /// Command and arguments to spawn.
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Sidecar an existing process by attaching to its controlling terminal.
    ///
    /// Example:
    ///   tuitap attach --pid 1234
    ///
    /// Note: requires read access to /proc/<pid>/fd/0 (Linux only).
    Attach {
        /// PID of the target process.
        #[arg(short, long)]
        pid: u32,
    },
}

// ── entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Initialise logging (RUST_LOG=info by default)
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // ── Load configuration ─────────────────────────────────────────────────
    let file_config: CliConfig = if cli.config.exists() {
        let raw = std::fs::read_to_string(&cli.config)?;
        toml::from_str(&raw)?
    } else {
        info!(
            "Config file {:?} not found — using defaults",
            cli.config
        );
        CliConfig::default()
    };

    let gateway_config: GatewayConfig = file_config.gateway.clone();
    let channels = file_config.channels.build_channels();

    // ── Build gateway + trust store ────────────────────────────────────────
    let mut gateway = Gateway::new(gateway_config.clone());
    for entry in &file_config.trust {
        gateway
            .trust_store
            .add(entry.id.clone(), entry.permission, entry.label.clone());
    }

    // ── Dispatch subcommand ────────────────────────────────────────────────
    match cli.command {
        Commands::Run { command } => {
            if command.is_empty() {
                return Err("run requires a command".into());
            }
            let prog = &command[0];
            let args: Vec<&str> = command[1..].iter().map(|s| s.as_str()).collect();
            run::run_command(prog, &args, gateway_config, channels, gateway).await?;
        }

        Commands::Attach { pid } => {
            attach::attach_pid(pid, gateway_config, channels, gateway).await?;
        }
    }

    Ok(())
}
