use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::PushError;

// ── Message types ─────────────────────────────────────────────────────────────

/// Priority level for outbound messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MessagePriority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

/// A message to be sent out through a push channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Optional title / subject line.
    pub title: Option<String>,
    /// Message body (plain text).
    pub body: String,
    /// Delivery priority hint.
    #[serde(default)]
    pub priority: MessagePriority,
}

impl OutboundMessage {
    pub fn new(body: impl Into<String>) -> Self {
        OutboundMessage {
            title: None,
            body: body.into(),
            priority: MessagePriority::Normal,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }
}

/// A message received from a remote client through an inbound channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// The identity of the sender (device token, bot user ID, etc.).
    pub from: String,
    /// Raw message text.
    pub body: String,
    /// Channel-specific opaque reference (e.g. Telegram `message_id`).
    pub channel_ref: String,
}

// ── Channel trait ─────────────────────────────────────────────────────────────

/// A pluggable outbound/inbound channel adapter.
///
/// Implementations must be object-safe so they can be stored as
/// `Box<dyn Channel>`.  `async-trait` boxes the futures to achieve this.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Send an outbound notification.
    async fn send(&self, message: &OutboundMessage) -> Result<(), PushError>;

    /// Poll for the next inbound message, returning `None` when nothing is
    /// available immediately.
    ///
    /// Send-only channels (Bark, Pushover) always return `Ok(None)`.
    async fn receive(&mut self) -> Result<Option<InboundMessage>, PushError>;
}
