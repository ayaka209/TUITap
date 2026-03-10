use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use crate::{
    channel::{Channel, InboundMessage, MessagePriority, OutboundMessage},
    error::PushError,
};

/// Configuration for the Pushover channel.
#[derive(Debug, Clone)]
pub struct PushoverConfig {
    /// Application API token.
    pub api_token: String,
    /// User key (or group key).
    pub user_key: String,
    /// Optional device name to restrict delivery.
    pub device: Option<String>,
}

impl PushoverConfig {
    pub fn new(api_token: impl Into<String>, user_key: impl Into<String>) -> Self {
        PushoverConfig {
            api_token: api_token.into(),
            user_key: user_key.into(),
            device: None,
        }
    }

    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }
}

/// Pushover push channel — **send-only**.
pub struct PushoverChannel {
    config: PushoverConfig,
    client: Client,
}

impl PushoverChannel {
    pub fn new(config: PushoverConfig) -> Self {
        PushoverChannel {
            config,
            client: Client::new(),
        }
    }
}

#[derive(Serialize)]
struct PushoverPayload<'a> {
    token: &'a str,
    user: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device: Option<&'a str>,
    priority: i8,
}

fn pushover_priority(p: MessagePriority) -> i8 {
    match p {
        MessagePriority::Low => -1,
        MessagePriority::Normal => 0,
        MessagePriority::High => 1,
        // "emergency" priority (2) requires retry/expire; use high instead.
        MessagePriority::Urgent => 1,
    }
}

#[async_trait]
impl Channel for PushoverChannel {
    fn name(&self) -> &str {
        "pushover"
    }

    async fn send(&self, message: &OutboundMessage) -> Result<(), PushError> {
        let payload = PushoverPayload {
            token: &self.config.api_token,
            user: &self.config.user_key,
            message: &message.body,
            title: message.title.as_deref(),
            device: self.config.device.as_deref(),
            priority: pushover_priority(message.priority),
        };

        let resp = self
            .client
            .post("https://api.pushover.net/1/messages.json")
            .json(&payload)
            .send()
            .await?;
        resp.error_for_status()?;
        Ok(())
    }

    /// Pushover is send-only; this always returns `Ok(None)`.
    async fn receive(&mut self) -> Result<Option<InboundMessage>, PushError> {
        Ok(None)
    }
}
