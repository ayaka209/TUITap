use tokio::sync::mpsc;
use tracing::info;

use tuitap_core::PtySession;
use tuitap_gateway::{Gateway, GatewayConfig};
use tuitap_push::Channel;

/// Spawn `command` under a fresh PTY and run the gateway event loop until
/// the child process exits.
pub async fn run_command(
    command: &str,
    args: &[&str],
    config: GatewayConfig,
    mut channels: Vec<Box<dyn Channel>>,
    mut gateway: Gateway,
) -> Result<(), Box<dyn std::error::Error>> {
    let cols = config.cols;
    let rows = config.rows;

    info!("Spawning `{command}` under PTY ({cols}×{rows})");

    let mut session = PtySession::spawn(command, args, cols, rows)?;
    let pty_reader = session.reader()?;

    // ── PTY write bridge ───────────────────────────────────────────────────
    // The gateway sends bytes through a tokio mpsc channel.
    // A std::thread drains that channel and writes to the PTY.
    let (tokio_write_tx, mut tokio_write_rx) = mpsc::channel::<Vec<u8>>(64);
    let (std_write_tx, std_write_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(64);

    // Bridge tokio mpsc → std mpsc
    tokio::spawn(async move {
        while let Some(data) = tokio_write_rx.recv().await {
            if std_write_tx.send(data).is_err() {
                break;
            }
        }
    });

    // Blocking thread: owns PtySession, drains writes, waits for child exit
    std::thread::spawn(move || {
        for data in std_write_rx.iter() {
            if session.write_input(&data).is_err() {
                break;
            }
        }
        let _ = session.wait();
    });

    // ── Gateway run ────────────────────────────────────────────────────────
    gateway
        .run(pty_reader, tokio_write_tx, &mut channels)
        .await?;

    info!("run_command: done");
    Ok(())
}

