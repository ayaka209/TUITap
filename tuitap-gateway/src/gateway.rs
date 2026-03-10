use std::io::Read;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use tuitap_core::{NoiseFilter, VteProcessor};
use tuitap_push::{Channel, InboundMessage, MessagePriority, OutboundMessage};

use crate::{
    approval::{ApprovalGate},
    config::GatewayConfig,
    dispatcher::Dispatcher,
    error::GatewayError,
    trust::{PermissionLevel, TrustStore},
};

// ── internal message types ───────────────────────────────────────────────────

/// Commands sent from the inbound channel tasks back to the main gateway loop.
enum InboundCmd {
    /// Forward `payload` bytes to PTY stdin.
    Forward {
        from: String,
        payload: Vec<u8>,
        #[allow(dead_code)]
        channel_ref: String,
    },
    /// Approve a pending approval request.
    Approve { from: String, approval_id: String },
    /// Reject a pending approval request.
    Reject { from: String, approval_id: String },
}

fn parse_cmd(msg: &InboundMessage) -> InboundCmd {
    let body = msg.body.trim();

    if let Some(rest) = body.strip_prefix("/approve ") {
        return InboundCmd::Approve {
            from: msg.from.clone(),
            approval_id: rest.trim().to_string(),
        };
    }
    if let Some(rest) = body.strip_prefix("/reject ") {
        return InboundCmd::Reject {
            from: msg.from.clone(),
            approval_id: rest.trim().to_string(),
        };
    }

    InboundCmd::Forward {
        from: msg.from.clone(),
        payload: msg.body.as_bytes().to_vec(),
        channel_ref: msg.channel_ref.clone(),
    }
}

// ── Gateway ──────────────────────────────────────────────────────────────────

/// The central hub that brokers all communication between the local PTY
/// session and remote push channels.
///
/// # Life-cycle
///
/// 1. Build a `Gateway` with [`Gateway::new`].
/// 2. Call [`Gateway::run`], passing an open PTY reader / writer and any
///    configured push channels.  `run` blocks until the PTY session ends.
pub struct Gateway {
    config: GatewayConfig,
    pub trust_store: TrustStore,
    pub approval_gate: ApprovalGate,
    dispatcher: Dispatcher,
    noise_filter: NoiseFilter,
}

impl Gateway {
    /// Create a new gateway from a configuration.
    pub fn new(config: GatewayConfig) -> Self {
        let cooldown = config.cooldown();
        Gateway {
            config,
            trust_store: TrustStore::new(),
            approval_gate: ApprovalGate::new(),
            dispatcher: Dispatcher::new(cooldown),
            noise_filter: NoiseFilter::default(),
        }
    }

    /// Run the gateway event loop.
    ///
    /// * `pty_reader` — blocking reader for PTY master output.
    /// * `pty_writer_tx` — async channel sender; bytes sent here are written
    ///   to the PTY's stdin by the caller.
    /// * `channels` — push channels to push outbound messages to and poll for
    ///   inbound messages.
    ///
    /// Returns when `pty_reader` reaches EOF (i.e. the wrapped process exits).
    pub async fn run(
        &mut self,
        pty_reader: Box<dyn Read + Send + 'static>,
        pty_writer_tx: mpsc::Sender<Vec<u8>>,
        channels: &mut Vec<Box<dyn Channel>>,
    ) -> Result<(), GatewayError> {
        let rows = self.config.rows as usize;
        let cols = self.config.cols as usize;

        // ── PTY reader task ────────────────────────────────────────────────
        let (pty_data_tx, mut pty_data_rx) = mpsc::channel::<Vec<u8>>(256);
        tokio::task::spawn_blocking(move || {
            let mut reader = pty_reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if pty_data_tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("PTY reader error: {e}");
                        break;
                    }
                }
            }
        });

        // ── Inbound channel polling tasks ──────────────────────────────────
        // For each channel that may have inbound messages, we spin up a
        // short-polling loop that sends parsed commands to the main loop.
        // We take the channels out of the vec so the tasks can own them.
        let (inbound_tx, mut inbound_rx) = mpsc::channel::<InboundMessage>(64);
        // Spawn a polling task per channel using indices.
        // We re-borrow `channels` mutably for each poll inside the loop so we
        // keep a single-threaded model (no per-channel tasks) to avoid the
        // lifetime/ownership complexity of boxing each channel into a separate
        // tokio task.

        // ── VTE processor ─────────────────────────────────────────────────
        let mut vte = VteProcessor::new(rows, cols);

        // ── Main event loop ────────────────────────────────────────────────
        loop {
            // Poll inbound channels (non-blocking)
            for ch in channels.iter_mut() {
                match ch.receive().await {
                    Ok(Some(msg)) => {
                        let _ = inbound_tx.try_send(msg);
                    }
                    Ok(None) => {}
                    Err(e) => warn!("inbound error from {}: {e}", ch.name()),
                }
            }

            tokio::select! {
                biased;

                // PTY output → process → maybe push
                Some(data) = pty_data_rx.recv() => {
                    vte.process(&data);
                    let snapshot = vte.snapshot();
                    let clean = self.noise_filter.filter(&snapshot);

                    if self.dispatcher.should_dispatch(&clean) && !clean.is_empty() {
                        let msg = OutboundMessage {
                            title: Some("Terminal update".into()),
                            body: clean,
                            priority: MessagePriority::Normal,
                        };
                        for ch in channels.iter() {
                            if let Err(e) = ch.send(&msg).await {
                                warn!("Failed to push to {}: {e}", ch.name());
                            }
                        }
                    }
                }

                // Inbound message from a channel
                Some(msg) = inbound_rx.recv() => {
                    self.handle_inbound(msg, &pty_writer_tx).await?;
                }

                // No more PTY data and no inbound — PTY has closed
                else => break,
            }
        }

        info!("Gateway: PTY session ended");
        Ok(())
    }

    // ── Private helpers ────────────────────────────────────────────────────

    async fn handle_inbound(
        &mut self,
        msg: InboundMessage,
        pty_tx: &mpsc::Sender<Vec<u8>>,
    ) -> Result<(), GatewayError> {
        // Check that we know this identity
        if self.trust_store.permission(&msg.from).is_none() {
            warn!(
                "Ignoring message from unknown identity '{}': {:?}",
                msg.from, msg.body
            );
            return Err(GatewayError::UnknownIdentity(msg.from));
        }

        let cmd = parse_cmd(&msg);

        match cmd {
            InboundCmd::Approve { from, approval_id } => {
                if !self
                    .trust_store
                    .has_permission(&from, PermissionLevel::Approve)
                {
                    return Err(GatewayError::PermissionDenied(from));
                }
                if let Some(payload) = self.approval_gate.respond(&approval_id, true) {
                    info!("Approval {approval_id} granted by {from}");
                    pty_tx
                        .send(payload)
                        .await
                        .map_err(|e| GatewayError::Send(e.to_string()))?;
                }
            }

            InboundCmd::Reject { from, approval_id } => {
                if !self
                    .trust_store
                    .has_permission(&from, PermissionLevel::Approve)
                {
                    return Err(GatewayError::PermissionDenied(from));
                }
                info!("Approval {approval_id} rejected by {from}");
                self.approval_gate.respond(&approval_id, false);
            }

            InboundCmd::Forward { from, payload, .. } => {
                if !self
                    .trust_store
                    .has_permission(&from, PermissionLevel::SendInput)
                {
                    return Err(GatewayError::PermissionDenied(from));
                }

                let input_str = String::from_utf8_lossy(&payload);

                // Check if any approval rule matches this input
                let needs_approval = self
                    .config
                    .approval_rules
                    .iter()
                    .any(|r| r.matches(&input_str));

                if needs_approval {
                    let desc = format!("Input from {from}: {input_str:?}");
                    let (id, _rx) =
                        self.approval_gate
                            .request(payload, from.clone(), desc.clone());
                    info!("Approval required for input from {from}: id={id}");
                    // The actual forwarding happens when an Approve-level
                    // identity sends `/approve <id>`.
                } else {
                    info!("Forwarding input from {from} to PTY");
                    pty_tx
                        .send(payload)
                        .await
                        .map_err(|e| GatewayError::Send(e.to_string()))?;
                }
            }
        }

        Ok(())
    }
}
