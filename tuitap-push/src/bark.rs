use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use crate::{
    channel::{Channel, InboundMessage, MessagePriority, OutboundMessage},
    error::PushError,
};

/// Configuration for the Bark channel.
#[derive(Debug, Clone)]
pub struct BarkConfig {
    /// Bark server base URL (default: `https://api.day.app`).
    pub server: String,
    /// Device-specific push key issued by the Bark app.
    pub device_key: String,
}

impl BarkConfig {
    pub fn new(device_key: impl Into<String>) -> Self {
        BarkConfig {
            server: "https://api.day.app".into(),
            device_key: device_key.into(),
        }
    }

    pub fn with_server(mut self, server: impl Into<String>) -> Self {
        self.server = server.into();
        self
    }
}

/// Bark push channel — **send-only** (iOS push notifications via Bark).
pub struct BarkChannel {
    config: BarkConfig,
    client: Client,
}

impl BarkChannel {
    pub fn new(config: BarkConfig) -> Self {
        BarkChannel {
            config,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct BarkPayload<'a> {
    device_key: &'a str,
    title: &'a str,
    body: &'a str,
    level: &'a str,
}

fn bark_level(p: MessagePriority) -> &'static str {
    match p {
        MessagePriority::Low => "passive",
        MessagePriority::Normal => "active",
        MessagePriority::High => "timeSensitive",
        MessagePriority::Urgent => "critical",
    }
}

#[async_trait]
impl Channel for BarkChannel {
    fn name(&self) -> &str {
        "bark"
    }

    async fn send(&self, message: &OutboundMessage) -> Result<(), PushError> {
        let url = format!(
            "{}/push",
            self.config.server.trim_end_matches('/')
        );
        let payload = BarkPayload {
            device_key: &self.config.device_key,
            title: message.title.as_deref().unwrap_or("TUITap"),
            body: &message.body,
            level: bark_level(message.priority),
        };

        let resp = self.client.post(&url).json(&payload).send().await?;
        resp.error_for_status()?;
        Ok(())
    }

    /// Bark is send-only; this always returns `Ok(None)`.
    async fn receive(&mut self) -> Result<Option<InboundMessage>, PushError> {
        Ok(None)
    }
}
