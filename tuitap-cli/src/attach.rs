use tokio::sync::mpsc;
use tracing::info;

use tuitap_core::pty::open_controlling_tty;
use tuitap_gateway::{Gateway, GatewayConfig};
use tuitap_push::Channel;

/// Attach to an existing process `pid` and run the gateway event loop.
///
/// ## Platform support
///
/// On **Linux**, this opens the controlling terminal of `pid` via
/// `/proc/<pid>/fd/0` and reads its output for the duration of the attach.
/// Writing input back is also supported when the process has a writable TTY.
///
/// On other platforms, an error is returned immediately.
pub async fn attach_pid(
    pid: u32,
    _config: GatewayConfig,
    mut channels: Vec<Box<dyn Channel>>,
    mut gateway: Gateway,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Attaching to pid {pid}");

    let tty_file = open_controlling_tty(pid)?;

    // Obtain read + write halves via dup(2)-style clone.
    let tty_reader = tty_file.try_clone()?;
    let tty_writer = tty_file;

    // ── PTY write bridge ───────────────────────────────────────────────────
    let (tokio_write_tx, mut tokio_write_rx) = mpsc::channel::<Vec<u8>>(64);
    let (std_write_tx, std_write_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(64);

    tokio::spawn(async move {
        while let Some(data) = tokio_write_rx.recv().await {
            if std_write_tx.send(data).is_err() {
                break;
            }
        }
    });

    std::thread::spawn(move || {
        use std::io::Write;
        let mut writer = tty_writer;
        for data in std_write_rx.iter() {
            if writer.write_all(&data).is_err() {
                break;
            }
        }
    });

    // ── Gateway run ────────────────────────────────────────────────────────
    gateway
        .run(Box::new(tty_reader), tokio_write_tx, &mut channels)
        .await?;

    info!("attach_pid {pid}: done");
    Ok(())
}
